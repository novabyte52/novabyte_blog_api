//! The estimated-tax engine.
//!
//! Deliberately pure: no database, no async, no I/O. Everything it needs is
//! passed in, so the whole thing is unit-testable against hand-computed
//! numbers — which matters, because a wrong answer here costs real money in
//! underpayment penalties.

use rust_decimal::Decimal;
use time::Date;

use crate::models::finance::{
    EstimateBasis, EstimateLine, ExpenseRecord, IncomeKind, IncomeRecord, Jurisdiction,
    JurisdictionDue, Projection, ProjectionMethod, Quarter, QuarterlyEstimate, TaxPayment,
    YtdTotals,
};
use crate::models::tax_info::{FilingStatus, Money, TaxProfile, TaxRules};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaxCalcError {
    MissingFederalBrackets(FilingStatus),
    MissingStateRules(String),
    MissingMunicipalRules(String),
}

impl std::fmt::Display for TaxCalcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaxCalcError::MissingFederalBrackets(s) => {
                write!(f, "no federal bracket schedule for filing status {:?}", s)
            }
            TaxCalcError::MissingStateRules(s) => write!(f, "no tax rules for state {}", s),
            TaxCalcError::MissingMunicipalRules(m) => {
                write!(f, "no tax rules for municipality {}", m)
            }
        }
    }
}

pub struct EstimateInputs<'a> {
    pub profile: &'a TaxProfile,
    pub rules: &'a TaxRules,
    pub income: &'a [IncomeRecord],
    pub expenses: &'a [ExpenseRecord],
    pub payments: &'a [TaxPayment],
    pub quarter: Quarter,
    pub method: ProjectionMethod,
}

const HALF: Decimal = Decimal::from_parts(5, 0, 0, false, 1); // 0.5

/// Sum the year-to-date ledger, project it to a full year, apply every layer
/// of tax, then net out what's already been paid in.
pub fn estimate(inputs: EstimateInputs) -> Result<QuarterlyEstimate, TaxCalcError> {
    let EstimateInputs {
        profile,
        rules,
        income,
        expenses,
        payments,
        quarter,
        method,
    } = inputs;

    let status = profile.filing_status;
    let year = profile.year;
    let period_end = quarter.period_end(year);
    let mut lines: Vec<EstimateLine> = Vec::new();

    // ---- 1. Year-to-date actuals -------------------------------------
    let in_period = |d: Date| d.year() == year.0 as i32 && d <= period_end;

    let wages: Money = income
        .iter()
        .filter(|r| r.kind == IncomeKind::Wage && in_period(r.date))
        .map(|r| r.amount)
        .sum();

    let se_gross: Money = income
        .iter()
        .filter(|r| r.kind == IncomeKind::SelfEmploymentProfit && in_period(r.date))
        .map(|r| r.amount)
        .sum();

    let se_deductible_expenses: Money = expenses
        .iter()
        .filter(|e| e.deductible && in_period(e.date))
        .map(|e| e.amount)
        .sum();

    let non_deductible_expenses: Money = expenses
        .iter()
        .filter(|e| !e.deductible && in_period(e.date))
        .map(|e| e.amount)
        .sum();

    let ytd = YtdTotals {
        wages,
        se_gross,
        se_deductible_expenses,
        // Signed: a genuine loss should reduce federal taxable income even
        // though it can't produce negative SE tax.
        se_net: se_gross - se_deductible_expenses,
        non_deductible_expenses,
    };

    lines.push(
        EstimateLine::new("W-2 wages received to date", ytd.wages)
            .with_detail(format!("through {}", period_end)),
    );
    lines.push(EstimateLine::new("LLC gross income to date", ytd.se_gross));
    lines.push(EstimateLine::new(
        "Deductible business expenses to date",
        ytd.se_deductible_expenses,
    ));
    lines.push(EstimateLine::new("Net LLC profit to date", ytd.se_net));

    // ---- 2. Project to a full year -----------------------------------
    let factor = match method {
        ProjectionMethod::YtdAsFinal => Decimal::ONE,
        ProjectionMethod::Annualized => {
            let elapsed = Decimal::from(quarter.days_elapsed(year));
            let total = Decimal::from(days_in_year(year.0));
            if elapsed.is_zero() {
                Decimal::ONE
            } else {
                total / elapsed
            }
        }
    };

    let scale = |m: Money| Money(m.0 * factor);
    let projected = YtdTotals {
        wages: scale(ytd.wages),
        se_gross: scale(ytd.se_gross),
        se_deductible_expenses: scale(ytd.se_deductible_expenses),
        se_net: scale(ytd.se_net),
        non_deductible_expenses: scale(ytd.non_deductible_expenses),
    };

    lines.push(
        EstimateLine::new("Projected annual W-2 wages", projected.wages).with_detail(
            match method {
                ProjectionMethod::Annualized => format!(
                    "year-to-date scaled by {}/{} days",
                    days_in_year(year.0),
                    quarter.days_elapsed(year)
                ),
                ProjectionMethod::YtdAsFinal => "year-to-date treated as final".to_string(),
            },
        ),
    );
    lines.push(EstimateLine::new(
        "Projected annual net LLC profit",
        projected.se_net,
    ));

    // ---- 3. Self-employment tax --------------------------------------
    let se_tax = rules
        .federal
        .se_tax
        .tax_owed(projected.se_net, projected.wages, status);

    lines.push(
        EstimateLine::new("Self-employment tax", se_tax).with_detail(format!(
            "on {} of net earnings; W-2 wages consume the Social Security base first",
            rules.federal.se_tax.net_earnings(projected.se_net).0
        )),
    );

    let half_se_tax = Money(se_tax.0 * HALF);
    lines.push(EstimateLine::new(
        "Deductible half of self-employment tax",
        half_se_tax,
    ));

    // ---- 4. Federal income tax ---------------------------------------
    let qbi_deduction = rules
        .federal
        .qbi_deduction_rate
        .apply(projected.se_net.max(Money::ZERO));

    let agi = projected.wages + projected.se_net - half_se_tax;
    lines.push(EstimateLine::new("Adjusted gross income", agi));

    let standard = rules.federal.standard_deduction_for(status);
    let itemized: Money = profile.deductions.iter().map(|d| d.amount).sum();

    // Default is whichever is larger — there is no reason to pay more tax by
    // accident. `prefer_itemized` is the explicit override for the cases where
    // itemizing is required regardless (a spouse itemizing on a separate return).
    let deduction = if profile.prefer_itemized {
        itemized
    } else {
        standard.max(itemized)
    };

    lines.push(
        EstimateLine::new("Deduction applied", deduction).with_detail(if profile.prefer_itemized {
            format!("itemized (forced); standard would be {}", standard.0)
        } else if itemized > standard {
            "itemized (larger than standard)".to_string()
        } else {
            "standard".to_string()
        }),
    );
    lines.push(EstimateLine::new("QBI deduction", qbi_deduction));

    let federal_taxable = agi.saturating_sub(deduction).saturating_sub(qbi_deduction);
    lines.push(EstimateLine::new("Federal taxable income", federal_taxable));

    let federal_brackets = rules
        .federal
        .brackets_for(status)
        .ok_or(TaxCalcError::MissingFederalBrackets(status))?;
    let federal_income_tax = federal_brackets.tax_owed(federal_taxable);
    lines.push(
        EstimateLine::new("Federal income tax", federal_income_tax).with_detail(format!(
            "marginal rate {}",
            federal_brackets.marginal_rate(federal_taxable).0
        )),
    );

    // ---- 5. State income tax -----------------------------------------
    let state_rules = rules
        .state_for(profile.state)
        .ok_or_else(|| TaxCalcError::MissingStateRules(profile.state.as_key().to_string()))?;

    let state_income_tax = match &state_rules.business_income_deduction {
        // Ohio's structure: business income is exempt up to a threshold, the
        // excess is taxed at a flat rate, and nonbusiness income runs through
        // the regular schedule.
        Some(bid) => {
            let business = projected.se_net.max(Money::ZERO);
            let exempt = business.min(bid.exempt_up_to);
            let taxed_flat = business.saturating_sub(bid.exempt_up_to);
            lines.push(
                EstimateLine::new("State business income deduction", exempt)
                    .with_detail(format!("exempt up to {}", bid.exempt_up_to.0)),
            );
            let nonbusiness_tax = state_rules.brackets.tax_owed(projected.wages);
            let business_tax = bid.rate_above_exemption.apply(taxed_flat);
            nonbusiness_tax + business_tax
        }
        None => state_rules
            .brackets
            .tax_owed(projected.wages + projected.se_net.max(Money::ZERO)),
    };
    lines.push(EstimateLine::new("State income tax", state_income_tax));

    // ---- 6. Municipal income tax -------------------------------------
    let municipal_income_tax = match &profile.city {
        Some(city) => {
            let muni = rules
                .municipal_for(city)
                .ok_or_else(|| TaxCalcError::MissingMunicipalRules(city.0.clone()))?;
            let base = projected.wages + projected.se_net.max(Money::ZERO);
            let tax = muni.flat_rate.apply(base);
            lines.push(
                EstimateLine::new(format!("{} municipal income tax", muni.name), tax)
                    .with_detail(format!("flat {} on {}", muni.flat_rate.0, base.0)),
            );
            tax
        }
        None => Money::ZERO,
    };

    let projected_annual = Projection {
        se_tax,
        federal_income_tax,
        state_income_tax,
        municipal_income_tax,
        total: se_tax + federal_income_tax + state_income_tax + municipal_income_tax,
    };
    lines.push(EstimateLine::new(
        "Total projected annual tax",
        projected_annual.total,
    ));

    // ---- 7. Safe harbor (federal only) -------------------------------
    let federal_projected = se_tax + federal_income_tax;
    let safe_harbor_annual = profile.prior_year_total_tax.map(|prior| {
        let threshold = rules
            .federal
            .safe_harbor_high_income_threshold
            .get(status.as_key())
            .copied();
        // The 110% multiplier is gated on *last* year's AGI, not this
        // year's projected `agi` computed above — using the current year's
        // figure here would apply the wrong year's income to a rule whose
        // whole purpose is to be independent of how this year turns out.
        // Unknown prior-year AGI defaults to "not high income" (the 100%
        // multiplier), matching how a missing threshold is already treated
        // below it — the safe-harbor guarantee simply doesn't upgrade to
        // 110% without the data to justify it.
        let high_income = match (threshold, profile.prior_year_agi) {
            (Some(t), Some(prior_agi)) => prior_agi > t,
            _ => false,
        };
        if high_income {
            rules.federal.safe_harbor_high_income_rate.apply(prior)
        } else {
            prior
        }
    });

    if let Some(sh) = safe_harbor_annual {
        lines.push(
            EstimateLine::new("Federal safe harbor", sh)
                .with_detail("based on prior-year total tax".to_string()),
        );
    }

    // Pay the lesser of the two: safe harbor blocks the underpayment penalty
    // even if this year's income ends up higher than projected.
    let (federal_basis_annual, federal_basis) = match safe_harbor_annual {
        Some(sh) if sh < federal_projected => (sh, EstimateBasis::SafeHarbor),
        _ => (federal_projected, EstimateBasis::CurrentYearProjection),
    };

    // ---- 8. Credits already applied ----------------------------------
    let withheld = |pick: fn(&crate::models::finance::Withholding) -> Money| -> Money {
        income
            .iter()
            .filter(|r| in_period(r.date))
            .filter_map(|r| r.withholding.as_ref().map(pick))
            .sum()
    };
    let federal_withholding = withheld(|w| w.federal);
    let state_withholding = withheld(|w| w.state);
    let local_withholding = withheld(|w| w.local);

    let paid = |j: Jurisdiction| -> Money {
        payments
            .iter()
            .filter(|p| p.year == year && p.jurisdiction == j && p.quarter <= quarter)
            .map(|p| p.amount)
            .sum()
    };

    // ---- 9. What's due for this period -------------------------------
    let fraction = quarter.cumulative_fraction();

    let build = |jurisdiction: Jurisdiction,
                 projected_tax: Money,
                 basis_annual: Money,
                 basis: EstimateBasis,
                 withholding: Money|
     -> JurisdictionDue {
        let required_to_date = Money(basis_annual.0 * fraction);
        let payments_made = paid(jurisdiction);
        let amount_due = required_to_date
            .saturating_sub(withholding)
            .saturating_sub(payments_made)
            .round_dollars();
        JurisdictionDue {
            jurisdiction,
            projected_annual_tax: projected_tax,
            basis,
            basis_annual,
            required_to_date,
            withholding_to_date: withholding,
            payments_made,
            amount_due,
        }
    };

    let due_by_jurisdiction = vec![
        build(
            Jurisdiction::Federal,
            federal_projected,
            federal_basis_annual,
            federal_basis,
            federal_withholding,
        ),
        build(
            Jurisdiction::State,
            state_income_tax,
            state_income_tax,
            EstimateBasis::CurrentYearProjection,
            state_withholding,
        ),
        build(
            Jurisdiction::Municipal,
            municipal_income_tax,
            municipal_income_tax,
            EstimateBasis::CurrentYearProjection,
            local_withholding,
        ),
    ];

    let federal_amount_due = due_by_jurisdiction[0].amount_due;
    let amount_due_this_quarter: Money = due_by_jurisdiction.iter().map(|d| d.amount_due).sum();

    for d in &due_by_jurisdiction {
        lines.push(
            EstimateLine::new(
                format!("{:?} due this quarter", d.jurisdiction),
                d.amount_due,
            )
            .with_detail(format!(
                "{} required to date less {} withheld and {} already paid",
                d.required_to_date.round_cents().0,
                d.withholding_to_date.0,
                d.payments_made.0
            )),
        );
    }

    Ok(QuarterlyEstimate {
        year,
        quarter,
        due_date: quarter.due_date(year),
        filing_status: status,
        projection_method: method,
        ytd,
        projected_annual_income: projected,
        projected_annual,
        safe_harbor_annual,
        due_by_jurisdiction,
        federal_amount_due,
        amount_due_this_quarter,
        lines,
    })
}

fn days_in_year(year: u16) -> i64 {
    let y = year as i32;
    if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
        366
    } else {
        365
    }
}

#[cfg(test)]
fn rate(s: &str) -> crate::models::tax_info::Rate {
    use std::str::FromStr;
    crate::models::tax_info::Rate(Decimal::from_str(s).expect("valid rate literal"))
}

#[cfg(test)]
fn dollars(units: i64) -> Money {
    Money(Decimal::from(units))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::finance::{Payer, Withholding};
    use crate::models::meta::Meta;
    use crate::models::tax_info::{
        Bracket, BracketSchedule, BusinessIncomeDeduction, Deduction, FederalRules, MunicipalRules,
        MunicipalityId, SelfEmploymentTaxRules, StateCode, StateRules, TaxYear,
    };
    use std::collections::HashMap;
    use time::macros::date;
    use time::OffsetDateTime;

    const YEAR: TaxYear = TaxYear(2026);

    fn meta() -> Meta<()> {
        Meta {
            id: "meta:x".into(),
            created_by: "person:x".into(),
            modified_by: None,
            deleted_by: None,
            data: None,
            created_on: OffsetDateTime::UNIX_EPOCH,
            modified_on: None,
            deleted_on: None,
        }
    }

    fn schedule(rungs: &[(Option<i64>, &str)]) -> BracketSchedule {
        BracketSchedule::new(
            rungs
                .iter()
                .map(|(ub, r)| Bracket {
                    upper_bound: ub.map(dollars),
                    rate: rate(r),
                })
                .collect(),
        )
        .expect("valid test schedule")
    }

    fn rules() -> TaxRules {
        let mut brackets = HashMap::new();
        brackets.insert(
            "single".to_string(),
            schedule(&[
                (Some(10_000), "0.10"),
                (Some(50_000), "0.20"),
                (None, "0.30"),
            ]),
        );

        let mut standard_deduction = HashMap::new();
        standard_deduction.insert("single".to_string(), dollars(15_000));

        let mut additional_medicare_threshold = HashMap::new();
        additional_medicare_threshold.insert("single".to_string(), dollars(200_000));

        let mut safe_harbor_high_income_threshold = HashMap::new();
        safe_harbor_high_income_threshold.insert("single".to_string(), dollars(150_000));

        let mut state = HashMap::new();
        state.insert(
            "OH".to_string(),
            StateRules {
                brackets: schedule(&[(Some(100_000), "0.02"), (None, "0.03")]),
                business_income_deduction: Some(BusinessIncomeDeduction {
                    exempt_up_to: dollars(125_000),
                    rate_above_exemption: rate("0.03"),
                }),
            },
        );

        let mut municipal = HashMap::new();
        municipal.insert(
            "toledo-oh".to_string(),
            MunicipalRules {
                name: "Toledo".to_string(),
                flat_rate: rate("0.025"),
            },
        );

        TaxRules {
            year: YEAR,
            federal: FederalRules {
                brackets,
                standard_deduction,
                qbi_deduction_rate: rate("0.20"),
                se_tax: SelfEmploymentTaxRules {
                    net_earnings_factor: rate("0.9235"),
                    social_security_rate: rate("0.124"),
                    social_security_wage_base: dollars(184_500),
                    medicare_rate: rate("0.029"),
                    additional_medicare_rate: rate("0.009"),
                    additional_medicare_threshold,
                },
                safe_harbor_high_income_threshold,
                safe_harbor_high_income_rate: rate("1.10"),
            },
            state,
            municipal,
        }
    }

    fn profile() -> TaxProfile {
        TaxProfile {
            id: "tax_profile:x".into(),
            person: "person:x".into(),
            year: YEAR,
            filing_status: FilingStatus::Single,
            state: StateCode::OH,
            city: Some(MunicipalityId("toledo-oh".into())),
            prefer_itemized: false,
            deductions: vec![],
            prior_year_total_tax: None,
            prior_year_agi: None,
            meta: meta(),
        }
    }

    fn wage(amount: i64, date: Date, federal_withheld: i64) -> IncomeRecord {
        IncomeRecord {
            id: "income_record:w".into(),
            person: "person:x".into(),
            kind: IncomeKind::Wage,
            payer: Payer::Employer {
                name: "Day Job".into(),
            },
            amount: dollars(amount),
            date,
            withholding: Some(Withholding {
                federal: dollars(federal_withheld),
                state: Money::ZERO,
                local: Money::ZERO,
            }),
            category: None,
            note: None,
            meta: meta(),
        }
    }

    fn llc(amount: i64, date: Date) -> IncomeRecord {
        IncomeRecord {
            id: "income_record:l".into(),
            person: "person:x".into(),
            kind: IncomeKind::SelfEmploymentProfit,
            payer: Payer::Client {
                name: "Client".into(),
            },
            amount: dollars(amount),
            date,
            withholding: None,
            category: None,
            note: None,
            meta: meta(),
        }
    }

    fn expense(amount: i64, date: Date, deductible: bool) -> ExpenseRecord {
        ExpenseRecord {
            id: "expense_record:e".into(),
            person: "person:x".into(),
            amount: dollars(amount),
            date,
            category: None,
            deductible,
            note: None,
            meta: meta(),
        }
    }

    fn run(
        profile: &TaxProfile,
        income: &[IncomeRecord],
        expenses: &[ExpenseRecord],
        payments: &[TaxPayment],
        quarter: Quarter,
    ) -> QuarterlyEstimate {
        estimate(EstimateInputs {
            profile,
            rules: &rules(),
            income,
            expenses,
            payments,
            quarter,
            method: ProjectionMethod::YtdAsFinal,
        })
        .expect("estimate should succeed")
    }

    // ---- bracket engine ----

    #[test]
    fn brackets_tax_each_slice_at_its_own_rate() {
        let s = schedule(&[
            (Some(10_000), "0.10"),
            (Some(50_000), "0.20"),
            (None, "0.30"),
        ]);
        // 10k @ 10% = 1000; next 40k @ 20% = 8000; 10k @ 30% = 3000
        assert_eq!(s.tax_owed(dollars(60_000)), dollars(12_000));
    }

    #[test]
    fn brackets_handle_exact_boundaries_and_zero() {
        let s = schedule(&[(Some(10_000), "0.10"), (None, "0.20")]);
        assert_eq!(s.tax_owed(Money::ZERO), Money::ZERO);
        assert_eq!(s.tax_owed(dollars(10_000)), dollars(1_000));
        assert_eq!(
            s.tax_owed(dollars(10_001)),
            Money(rust_decimal::Decimal::from(1_000) + rust_decimal::Decimal::new(2, 1))
        );
    }

    #[test]
    fn schedule_rejects_malformed_input() {
        assert!(BracketSchedule::new(vec![]).is_err());
        // no open-ended top bracket
        assert!(BracketSchedule::new(vec![Bracket {
            upper_bound: Some(dollars(10)),
            rate: rate("0.1")
        }])
        .is_err());
        // descending bounds
        assert!(BracketSchedule::new(vec![
            Bracket {
                upper_bound: Some(dollars(100)),
                rate: rate("0.1")
            },
            Bracket {
                upper_bound: Some(dollars(50)),
                rate: rate("0.2")
            },
            Bracket {
                upper_bound: None,
                rate: rate("0.3")
            },
        ])
        .is_err());
    }

    // ---- self-employment tax ----

    #[test]
    fn w2_wages_consume_the_social_security_wage_base_first() {
        let se = rules().federal.se_tax;
        // Wages already exceed the base, so no SE income can be hit by the
        // Social Security portion — only Medicare applies.
        let tax = se.tax_owed(dollars(100_000), dollars(200_000), FilingStatus::Single);
        let net_earnings = se.net_earnings(dollars(100_000));
        let medicare = rate("0.029").apply(net_earnings);
        // Wages of 200k already exceed the 200k Additional Medicare threshold,
        // so all SE earnings carry the surtax too.
        let surtax = rate("0.009").apply(net_earnings);
        assert_eq!(tax, medicare + surtax);
    }

    #[test]
    fn se_tax_charges_social_security_when_there_are_no_wages() {
        let se = rules().federal.se_tax;
        let tax = se.tax_owed(dollars(50_000), Money::ZERO, FilingStatus::Single);
        let net_earnings = se.net_earnings(dollars(50_000));
        let expected = rate("0.124").apply(net_earnings) + rate("0.029").apply(net_earnings);
        assert_eq!(tax, expected);
    }

    #[test]
    fn additional_medicare_counts_wages_toward_the_threshold() {
        let se = rules().federal.se_tax;
        // 190k wages leaves only 10k of headroom below the 200k threshold, so
        // SE earnings above that 10k carry the surtax. Computing the threshold
        // against SE earnings alone would wrongly charge nothing here.
        let tax = se.tax_owed(dollars(50_000), dollars(190_000), FilingStatus::Single);
        let net_earnings = se.net_earnings(dollars(50_000));
        let surtax_base = net_earnings.saturating_sub(dollars(10_000));
        assert!(surtax_base > Money::ZERO, "surtax should apply");
        let expected = rate("0.029").apply(net_earnings) + rate("0.009").apply(surtax_base);
        assert_eq!(tax, expected);
    }

    #[test]
    fn a_loss_produces_no_se_tax() {
        let se = rules().federal.se_tax;
        assert_eq!(
            se.tax_owed(
                Money(Decimal::from(-5_000)),
                Money::ZERO,
                FilingStatus::Single
            ),
            Money::ZERO
        );
    }

    // ---- end to end ----

    #[test]
    fn expenses_reduce_net_profit_and_therefore_tax() {
        let income = vec![llc(60_000, date!(2026 - 02 - 01))];
        let with_expenses = vec![expense(20_000, date!(2026 - 02 - 15), true)];

        let bare = run(&profile(), &income, &[], &[], Quarter::Q4);
        let net = run(&profile(), &income, &with_expenses, &[], Quarter::Q4);

        assert_eq!(bare.ytd.se_net, dollars(60_000));
        assert_eq!(net.ytd.se_net, dollars(40_000));
        assert!(net.projected_annual.total < bare.projected_annual.total);
    }

    #[test]
    fn non_deductible_expenses_do_not_change_the_tax() {
        let income = vec![llc(60_000, date!(2026 - 02 - 01))];
        let bare = run(&profile(), &income, &[], &[], Quarter::Q4);
        let with_personal = run(
            &profile(),
            &income,
            &[expense(20_000, date!(2026 - 02 - 15), false)],
            &[],
            Quarter::Q4,
        );

        assert_eq!(with_personal.ytd.non_deductible_expenses, dollars(20_000));
        assert_eq!(with_personal.ytd.se_net, bare.ytd.se_net);
        assert_eq!(
            with_personal.projected_annual.total,
            bare.projected_annual.total
        );
    }

    #[test]
    fn records_after_the_period_end_are_excluded() {
        let income = vec![
            llc(10_000, date!(2026 - 02 - 01)),
            llc(90_000, date!(2026 - 11 - 01)),
        ];
        let q1 = run(&profile(), &income, &[], &[], Quarter::Q1);
        assert_eq!(q1.ytd.se_gross, dollars(10_000));

        let q4 = run(&profile(), &income, &[], &[], Quarter::Q4);
        assert_eq!(q4.ytd.se_gross, dollars(100_000));
    }

    #[test]
    fn payments_already_made_reduce_the_amount_due() {
        let income = vec![llc(80_000, date!(2026 - 01 - 15))];
        let before = run(&profile(), &income, &[], &[], Quarter::Q3);

        let payment = TaxPayment {
            id: "tax_payment:p".into(),
            person: "person:x".into(),
            year: YEAR,
            quarter: Quarter::Q1,
            jurisdiction: Jurisdiction::Federal,
            amount: dollars(3_000),
            date: date!(2026 - 04 - 15),
            note: None,
            meta: meta(),
        };
        let after = run(&profile(), &income, &[], &[payment], Quarter::Q3);

        assert_eq!(
            after.federal_amount_due,
            before.federal_amount_due.saturating_sub(dollars(3_000))
        );
    }

    #[test]
    fn withholding_offsets_only_its_own_jurisdiction() {
        let income = vec![
            wage(80_000, date!(2026 - 06 - 01), 12_000),
            llc(40_000, date!(2026 - 06 - 01)),
        ];
        let est = run(&profile(), &income, &[], &[], Quarter::Q4);

        let federal = &est.due_by_jurisdiction[0];
        let municipal = &est.due_by_jurisdiction[2];
        assert_eq!(federal.withholding_to_date, dollars(12_000));
        // Federal withholding must not silently cover the Toledo liability.
        assert_eq!(municipal.withholding_to_date, Money::ZERO);
        assert!(municipal.amount_due > Money::ZERO);
    }

    #[test]
    fn safe_harbor_wins_when_it_is_the_smaller_number() {
        let income = vec![llc(150_000, date!(2026 - 03 - 01))];
        let mut p = profile();
        p.prior_year_total_tax = Some(dollars(5_000));

        let est = run(&p, &income, &[], &[], Quarter::Q4);
        let federal = &est.due_by_jurisdiction[0];

        assert_eq!(federal.basis, EstimateBasis::SafeHarbor);
        assert_eq!(federal.basis_annual, dollars(5_000));
        assert_eq!(est.safe_harbor_annual, Some(dollars(5_000)));
    }

    #[test]
    fn safe_harbor_is_ignored_when_the_projection_is_lower() {
        let income = vec![llc(20_000, date!(2026 - 03 - 01))];
        let mut p = profile();
        p.prior_year_total_tax = Some(dollars(90_000));

        let est = run(&p, &income, &[], &[], Quarter::Q4);
        assert_eq!(
            est.due_by_jurisdiction[0].basis,
            EstimateBasis::CurrentYearProjection
        );
    }

    #[test]
    fn high_income_safe_harbor_uses_the_110_percent_rate() {
        let income = vec![llc(300_000, date!(2026 - 03 - 01))];
        let mut p = profile();
        p.prior_year_total_tax = Some(dollars(40_000));
        // Prior-year AGI above the 150k high-income threshold — this, not
        // this year's income, is what should decide the multiplier.
        p.prior_year_agi = Some(dollars(200_000));

        let est = run(&p, &income, &[], &[], Quarter::Q4);
        assert_eq!(est.safe_harbor_annual, Some(dollars(44_000)));
    }

    #[test]
    fn safe_harbor_multiplier_follows_prior_year_agi_not_this_years() {
        // A strong current year must NOT trigger the 110% multiplier on its
        // own — only a high prior-year AGI does. Using this year's AGI here
        // would silently understate the safe-harbor amount.
        let strong_current_year = vec![llc(300_000, date!(2026 - 03 - 01))];
        let mut low_prior_agi = profile();
        low_prior_agi.prior_year_total_tax = Some(dollars(40_000));
        low_prior_agi.prior_year_agi = Some(dollars(50_000));

        let est = run(&low_prior_agi, &strong_current_year, &[], &[], Quarter::Q4);
        assert_eq!(
            est.safe_harbor_annual,
            Some(dollars(40_000)),
            "100% multiplier expected when prior-year AGI is below the threshold"
        );

        // Conversely, a weak current year with a high prior-year AGI must
        // still get the 110% multiplier.
        let weak_current_year = vec![llc(20_000, date!(2026 - 03 - 01))];
        let mut high_prior_agi = profile();
        high_prior_agi.prior_year_total_tax = Some(dollars(40_000));
        high_prior_agi.prior_year_agi = Some(dollars(200_000));

        let est2 = run(&high_prior_agi, &weak_current_year, &[], &[], Quarter::Q4);
        assert_eq!(
            est2.safe_harbor_annual,
            Some(dollars(44_000)),
            "110% multiplier expected when prior-year AGI is above the threshold"
        );
    }

    #[test]
    fn safe_harbor_defaults_to_100_percent_when_prior_year_agi_is_unknown() {
        let income = vec![llc(300_000, date!(2026 - 03 - 01))];
        let mut p = profile();
        p.prior_year_total_tax = Some(dollars(40_000));
        // prior_year_agi left None.

        let est = run(&p, &income, &[], &[], Quarter::Q4);
        assert_eq!(est.safe_harbor_annual, Some(dollars(40_000)));
    }

    #[test]
    fn quarterly_installments_accumulate_one_quarter_at_a_time() {
        let income = vec![llc(100_000, date!(2026 - 01 - 05))];
        let q1 = run(&profile(), &income, &[], &[], Quarter::Q1);
        let q2 = run(&profile(), &income, &[], &[], Quarter::Q2);
        let q4 = run(&profile(), &income, &[], &[], Quarter::Q4);

        let annual = q4.due_by_jurisdiction[0].basis_annual;
        assert_eq!(
            q1.due_by_jurisdiction[0].required_to_date,
            Money(annual.0 / Decimal::from(4))
        );
        assert_eq!(
            q2.due_by_jurisdiction[0].required_to_date,
            Money(annual.0 / Decimal::from(2))
        );
        assert_eq!(q4.due_by_jurisdiction[0].required_to_date, annual);
    }

    #[test]
    fn annualizing_scales_a_partial_year_up() {
        let income = vec![llc(25_000, date!(2026 - 02 - 01))];
        let annualized = estimate(EstimateInputs {
            profile: &profile(),
            rules: &rules(),
            income: &income,
            expenses: &[],
            payments: &[],
            quarter: Quarter::Q1,
            method: ProjectionMethod::Annualized,
        })
        .unwrap();

        // 90 days elapsed of 365 → roughly 4x
        assert!(annualized.projected_annual_income.se_net > dollars(100_000));
        assert!(annualized.projected_annual_income.se_net < dollars(102_000));
    }

    #[test]
    fn ohio_business_income_deduction_exempts_the_first_slice() {
        // Below the 125k exemption, business income owes no Ohio tax at all.
        let income = vec![llc(100_000, date!(2026 - 03 - 01))];
        let est = run(&profile(), &income, &[], &[], Quarter::Q4);
        assert_eq!(est.projected_annual.state_income_tax, Money::ZERO);

        // Above it, only the excess is taxed, at the flat 3%.
        let bigger = vec![llc(225_000, date!(2026 - 03 - 01))];
        let est2 = run(&profile(), &bigger, &[], &[], Quarter::Q4);
        assert_eq!(est2.projected_annual.state_income_tax, dollars(3_000));
    }

    #[test]
    fn toledo_taxes_wages_and_profit_at_the_flat_rate() {
        let income = vec![
            wage(50_000, date!(2026 - 03 - 01), 0),
            llc(50_000, date!(2026 - 03 - 01)),
        ];
        let est = run(&profile(), &income, &[], &[], Quarter::Q4);
        assert_eq!(est.projected_annual.municipal_income_tax, dollars(2_500));
    }

    #[test]
    fn itemized_deductions_are_used_when_larger() {
        let income = vec![llc(100_000, date!(2026 - 03 - 01))];
        let mut p = profile();
        p.deductions = vec![Deduction {
            label: "Mortgage interest".into(),
            amount: dollars(30_000),
        }];

        let standard = run(&profile(), &income, &[], &[], Quarter::Q4);
        let itemized = run(&p, &income, &[], &[], Quarter::Q4);

        assert!(
            itemized.projected_annual.federal_income_tax
                < standard.projected_annual.federal_income_tax
        );
    }

    #[test]
    fn missing_municipal_rules_are_an_error_not_a_wrong_number() {
        let mut p = profile();
        p.city = Some(MunicipalityId("nowhere-oh".into()));
        let err = estimate(EstimateInputs {
            profile: &p,
            rules: &rules(),
            income: &[llc(10_000, date!(2026 - 03 - 01))],
            expenses: &[],
            payments: &[],
            quarter: Quarter::Q1,
            method: ProjectionMethod::YtdAsFinal,
        })
        .unwrap_err();
        assert_eq!(
            err,
            TaxCalcError::MissingMunicipalRules("nowhere-oh".into())
        );
    }

    /// Full worked scenario with every figure computed by hand, so a
    /// regression anywhere in the chain trips this test.
    #[test]
    fn end_to_end_w2_plus_llc() {
        // $90,000 W-2 wages (federal withholding $10,000), $60,000 LLC gross,
        // $10,000 deductible expenses → $50,000 net LLC profit.
        let income = vec![
            wage(90_000, date!(2026 - 06 - 01), 10_000),
            llc(60_000, date!(2026 - 06 - 01)),
        ];
        let expenses = vec![expense(10_000, date!(2026 - 06 - 15), true)];
        let est = run(&profile(), &income, &expenses, &[], Quarter::Q4);

        assert_eq!(est.ytd.wages, dollars(90_000));
        assert_eq!(est.ytd.se_net, dollars(50_000));

        // SE tax: net earnings = 50,000 × 0.9235 = 46,175.
        // Wages of 90,000 leave 94,500 of the 184,500 SS base, so all of it is
        // subject to Social Security. No Additional Medicare (90,000 + 46,175
        // is under 200,000).
        let net_earnings = Money(Decimal::from(46_175));
        let expected_se = rate("0.124").apply(net_earnings) + rate("0.029").apply(net_earnings);
        assert_eq!(est.projected_annual.se_tax, expected_se);

        // AGI = 90,000 + 50,000 − half SE tax.
        let half_se = Money(expected_se.0 * HALF);
        let agi = dollars(140_000) - half_se;

        // Taxable = AGI − 15,000 standard − 20% QBI on 50,000 (= 10,000).
        let taxable = agi
            .saturating_sub(dollars(15_000))
            .saturating_sub(dollars(10_000));
        let expected_federal = rules()
            .federal
            .brackets_for(FilingStatus::Single)
            .unwrap()
            .tax_owed(taxable);
        assert_eq!(est.projected_annual.federal_income_tax, expected_federal);

        // Ohio: 50,000 of business income is under the 125,000 exemption, so
        // only the 90,000 of wages runs through the schedule at 2%.
        assert_eq!(est.projected_annual.state_income_tax, dollars(1_800));

        // Toledo: 2.5% of (90,000 + 50,000).
        assert_eq!(est.projected_annual.municipal_income_tax, dollars(3_500));

        // Federal due for Q4 = full-year federal liability less the 10,000
        // already withheld.
        let federal = &est.due_by_jurisdiction[0];
        assert_eq!(federal.basis, EstimateBasis::CurrentYearProjection);
        assert_eq!(
            federal.amount_due,
            (expected_se + expected_federal)
                .saturating_sub(dollars(10_000))
                .round_dollars()
        );

        assert_eq!(
            est.amount_due_this_quarter,
            federal.amount_due + dollars(1_800) + dollars(3_500)
        );
    }
}
