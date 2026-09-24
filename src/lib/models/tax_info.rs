use std::collections::HashMap;
use std::ops::{Add, Sub};

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------
// Primitives
// ---------------------------------------------------------------------

/// Money as fixed-point decimal. Never use f64 for currency — rounding
/// drift compounds across quarterly calculations.
///
/// Serialized as a decimal *string*. Every value read back out of SurrealDB
/// passes through `serde_json` in [`NovaResponse`](crate::db::nova_db::NovaResponse),
/// and a bare `Decimal` would serialize as a JSON number — which is an f64 on
/// the way back in. The string round-trip is what keeps the cents exact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
pub struct Money(#[serde(with = "rust_decimal::serde::str")] pub Decimal);

impl Money {
    pub const ZERO: Money = Money(Decimal::ZERO);

    /// Subtraction floored at zero — the shape almost every tax rule wants
    /// ("the amount above the threshold", "what's left of the wage base").
    pub fn saturating_sub(self, rhs: Money) -> Money {
        Money((self.0 - rhs.0).max(Decimal::ZERO))
    }

    pub fn min(self, rhs: Money) -> Money {
        Money(self.0.min(rhs.0))
    }

    pub fn max(self, rhs: Money) -> Money {
        Money(self.0.max(rhs.0))
    }

    /// Round to whole cents, half away from zero. Apply at presentation
    /// boundaries only — never between intermediate steps of a calculation.
    pub fn round_cents(self) -> Money {
        Money(self.0.round_dp(2))
    }

    /// Round to whole dollars — how the IRS wants 1040-ES lines reported.
    pub fn round_dollars(self) -> Money {
        Money(self.0.round_dp(0))
    }

    pub fn is_zero(self) -> bool {
        self.0.is_zero()
    }
}

impl Add for Money {
    type Output = Money;
    fn add(self, rhs: Money) -> Money {
        Money(self.0 + rhs.0)
    }
}

impl Sub for Money {
    type Output = Money;
    fn sub(self, rhs: Money) -> Money {
        Money(self.0 - rhs.0)
    }
}

impl std::iter::Sum for Money {
    fn sum<I: Iterator<Item = Money>>(iter: I) -> Money {
        iter.fold(Money::ZERO, |acc, m| acc + m)
    }
}

/// A rate like 0.153 (15.3%) or 0.0275 (2.75%). Same underlying type as
/// Money but kept distinct so you can't accidentally multiply two rates
/// together or add a rate to a dollar amount without the type system
/// complaining.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
pub struct Rate(#[serde(with = "rust_decimal::serde::str")] pub Decimal);

impl Rate {
    pub fn apply(&self, amount: Money) -> Money {
        Money(amount.0 * self.0)
    }
}

/// The tax year a given rule set applies to. A newtype instead of a bare
/// u16 so it can't be confused with, say, a filing year offset.
///
/// Deserialization is validated (see the manual `Deserialize` impl below)
/// rather than left to the derive: every quarter/date calculation downstream
/// (`Quarter::period_end`/`due_date`/`days_elapsed`) builds a `time::Date`
/// from this value and `.expect()`s the result, since a malformed tax year
/// is a client-input problem, not a runtime possibility worth threading a
/// `Result` through the calculation engine for. Validating once here, at
/// the one boundary every `TaxYear` is constructed from client input
/// through, is what makes that `.expect()` actually safe — a raw `u16`
/// goes up to 65535, far outside any year `time::Date` can represent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct TaxYear(pub u16);

impl<'de> Deserialize<'de> for TaxYear {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let year = u16::deserialize(d)?;
        // `time::Date` represents years 0..=9999; a four-digit calendar year
        // is the only value that means anything here anyway.
        if !(1900..=9999).contains(&year) {
            return Err(serde::de::Error::custom(format!(
                "tax year must be between 1900 and 9999, got {year}"
            )));
        }
        Ok(TaxYear(year))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FilingStatus {
    Single,
    MarriedFilingJointly,
    MarriedFilingSeparately,
    HeadOfHousehold,
}

impl FilingStatus {
    /// Stable string key for the per-status maps below.
    ///
    /// Those maps cross the `serde_json` boundary on every DB read, and JSON
    /// object keys must be strings — so the maps are keyed by these codes
    /// rather than by the enum itself.
    pub fn as_key(&self) -> &'static str {
        match self {
            FilingStatus::Single => "single",
            FilingStatus::MarriedFilingJointly => "mfj",
            FilingStatus::MarriedFilingSeparately => "mfs",
            FilingStatus::HeadOfHousehold => "hoh",
        }
    }
}

// ---------------------------------------------------------------------
// Per-person, per-year tax profile
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxProfile {
    pub id: String,
    pub person: String,
    pub year: TaxYear,
    pub filing_status: FilingStatus,
    pub state: StateCode,
    pub city: Option<MunicipalityId>,
    pub prefer_itemized: bool,
    pub deductions: Vec<Deduction>,
    /// Last year's total tax liability. Enables the safe-harbor estimate —
    /// pay 100% (110% for high earners) of the prior year and the
    /// underpayment penalty can't reach you regardless of how this year lands.
    pub prior_year_total_tax: Option<Money>,
    /// Last year's AGI — the figure the IRS actually keys the 110% "high
    /// income" safe-harbor multiplier on. Deliberately separate from *this*
    /// year's projected AGI (computed fresh every estimate): using the
    /// current year's number here would gate the multiplier on the wrong
    /// year's income and could silently understate the safe-harbor amount
    /// for the filers it exists to protect.
    pub prior_year_agi: Option<Money>,
    pub meta: super::meta::Meta<()>,
}

/// The write side of [`TaxProfile`] — everything except server-owned fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertTaxProfileArgs {
    pub filing_status: FilingStatus,
    pub state: StateCode,
    pub city: Option<MunicipalityId>,
    pub prefer_itemized: bool,
    pub deductions: Vec<Deduction>,
    pub prior_year_total_tax: Option<Money>,
    pub prior_year_agi: Option<Money>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deduction {
    pub label: String,
    pub amount: Money,
}

// ---------------------------------------------------------------------
// Bracket schedule — the shared engine for progressive tax tables
// (federal brackets, and Ohio's above-threshold structure both fit this)
// ---------------------------------------------------------------------

/// One rung of a progressive schedule. `upper_bound: None` marks the top
/// bracket (rate applies to everything above the previous bound).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bracket {
    pub upper_bound: Option<Money>,
    pub rate: Rate,
}

/// Invariant-enforcing wrapper: brackets are always sorted ascending by
/// `upper_bound`, and exactly one (the last) has `upper_bound: None`.
/// Building this through `BracketSchedule::new` instead of exposing the
/// `Vec<Bracket>` directly means bad config data fails at load time, not
/// silently at calculation time.
#[derive(Debug, Clone, Serialize)]
pub struct BracketSchedule(Vec<Bracket>);

/// Deserialization runs the same validation as [`BracketSchedule::new`], so a
/// malformed schedule in the `tax_rules` table is rejected at read time rather
/// than quietly producing a wrong number.
impl<'de> Deserialize<'de> for BracketSchedule {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let brackets = Vec::<Bracket>::deserialize(d)?;
        BracketSchedule::new(brackets).map_err(serde::de::Error::custom)
    }
}

impl BracketSchedule {
    pub fn new(brackets: Vec<Bracket>) -> Result<Self, &'static str> {
        if brackets.is_empty() {
            return Err("schedule must have at least one bracket");
        }
        let open_ended_count = brackets.iter().filter(|b| b.upper_bound.is_none()).count();
        if open_ended_count != 1 {
            return Err("schedule must have exactly one open-ended top bracket");
        }
        if !brackets.last().unwrap().upper_bound.is_none() {
            return Err("open-ended bracket must be last");
        }
        let bounded: Vec<_> = brackets.iter().filter_map(|b| b.upper_bound).collect();
        if !bounded.windows(2).all(|w| w[0] < w[1]) {
            return Err("bounded brackets must be strictly ascending");
        }
        Ok(Self(brackets))
    }

    /// Total tax owed on `taxable_income`, walking each rung.
    pub fn tax_owed(&self, taxable_income: Money) -> Money {
        let mut owed = Decimal::ZERO;
        let mut lower = Decimal::ZERO;
        for bracket in &self.0 {
            let upper = bracket.upper_bound.map(|m| m.0).unwrap_or(taxable_income.0);
            if taxable_income.0 <= lower {
                break;
            }
            let slice = (taxable_income.0.min(upper) - lower).max(Decimal::ZERO);
            owed += slice * bracket.rate.0;
            lower = upper;
        }
        Money(owed)
    }

    pub fn marginal_rate(&self, taxable_income: Money) -> Rate {
        for bracket in &self.0 {
            if bracket.upper_bound.is_none_or(|ub| taxable_income < ub) {
                return bracket.rate;
            }
        }
        self.0.last().unwrap().rate
    }

    pub fn brackets(&self) -> &[Bracket] {
        &self.0
    }
}

// ---------------------------------------------------------------------
// Self-employment tax — federal, but structurally distinct from the
// bracket ladder, so it's its own struct rather than forced into one
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfEmploymentTaxRules {
    pub net_earnings_factor: Rate,        // 0.9235
    pub social_security_rate: Rate,       // 0.124
    pub social_security_wage_base: Money, // e.g. $184,500 for 2026
    pub medicare_rate: Rate,              // 0.029
    pub additional_medicare_rate: Rate,   // 0.009
    /// Keyed by [`FilingStatus::as_key`].
    pub additional_medicare_threshold: HashMap<String, Money>,
}

impl SelfEmploymentTaxRules {
    /// Net earnings from self-employment: the 92.35% of net profit that
    /// SE tax actually applies to.
    pub fn net_earnings(&self, net_se_profit: Money) -> Money {
        self.net_earnings_factor
            .apply(net_se_profit.max(Money::ZERO))
    }

    /// `other_wages` lets you account for W-2 Social Security withholding
    /// already eating into the wage base — the exact wrinkle from your
    /// W-2-plus-side-income scenario.
    pub fn tax_owed(
        &self,
        net_se_profit: Money,
        other_wages: Money,
        filing_status: FilingStatus,
    ) -> Money {
        let taxable_earnings = self.net_earnings(net_se_profit);

        if taxable_earnings.is_zero() {
            return Money::ZERO;
        }

        // W-2 wages consume the Social Security wage base first.
        let remaining_ss_base = self.social_security_wage_base.saturating_sub(other_wages);
        let ss_taxable = taxable_earnings.min(remaining_ss_base);
        let ss_tax = self.social_security_rate.apply(ss_taxable);

        let medicare_tax = self.medicare_rate.apply(taxable_earnings);

        // The Additional Medicare threshold applies to *combined* wages and SE
        // earnings, not SE earnings alone. Wages fill the threshold first, so
        // only the part of SE earnings sitting above the remaining headroom is
        // subject to the surtax here — the employer already withholds on the
        // wage portion.
        // `unwrap_or(ZERO)` is unreachable in practice — `FederalRules`'s
        // `Deserialize` impl rejects any `TaxRules` missing an entry here for
        // one of the four filing statuses — but `HashMap::get` still returns
        // `Option`, so a fallback has to exist. ZERO threshold means ZERO
        // headroom, taxing the full amount at the surtax rate — overpaying
        // rather than underpaying is the safer of two wrong answers here.
        let threshold = self
            .additional_medicare_threshold
            .get(filing_status.as_key())
            .copied()
            .unwrap_or(Money::ZERO);
        let headroom = threshold.saturating_sub(other_wages);
        let excess = taxable_earnings.saturating_sub(headroom);
        let additional_medicare = self.additional_medicare_rate.apply(excess);

        ss_tax + medicare_tax + additional_medicare
    }
}

// ---------------------------------------------------------------------
// Per-jurisdiction rule sets
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StateCode {
    OH,
    // add as needed
}

impl StateCode {
    pub fn as_key(&self) -> &'static str {
        match self {
            StateCode::OH => "OH",
        }
    }
}

/// Slug identifying a municipality, e.g. `"toledo-oh"`. A string rather than
/// an opaque integer because it doubles as the key of
/// [`TaxRules::municipal`] — which has to be a string across the JSON hop
/// anyway — and reads far better in the seed file.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MunicipalityId(pub String);

#[derive(Debug, Clone, Serialize)]
pub struct FederalRules {
    /// Keyed by [`FilingStatus::as_key`].
    pub brackets: HashMap<String, BracketSchedule>,
    /// Keyed by [`FilingStatus::as_key`].
    pub standard_deduction: HashMap<String, Money>,
    pub qbi_deduction_rate: Rate, // 0.20
    pub se_tax: SelfEmploymentTaxRules,
    /// AGI above which safe harbor requires 110% of the prior year's tax
    /// instead of 100%. Keyed by [`FilingStatus::as_key`].
    pub safe_harbor_high_income_threshold: HashMap<String, Money>,
    pub safe_harbor_high_income_rate: Rate, // 1.10
}

/// Every entry in a per-filing-status map here has to cover all four
/// statuses, checked once at deserialize time below. A missing entry isn't
/// a detail worth shrugging off with a default: a missing safe-harbor
/// threshold silently disabled the 110% multiplier, while a missing
/// Additional Medicare threshold silently taxed *all* of a filer's SE
/// earnings at the surtax rate — two different wrong directions from the
/// same kind of gap. Rejecting incomplete rules at load time, the same way
/// [`BracketSchedule`] already validates itself, closes both at once.
fn require_all_filing_statuses<T>(map: &HashMap<String, T>, field: &str) -> Result<(), String> {
    let missing: Vec<&str> = [
        FilingStatus::Single,
        FilingStatus::MarriedFilingJointly,
        FilingStatus::MarriedFilingSeparately,
        FilingStatus::HeadOfHousehold,
    ]
    .iter()
    .map(FilingStatus::as_key)
    .filter(|key| !map.contains_key(*key))
    .collect();

    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{field} is missing entries for filing status: {}",
            missing.join(", ")
        ))
    }
}

impl<'de> Deserialize<'de> for FederalRules {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Shadow {
            brackets: HashMap<String, BracketSchedule>,
            standard_deduction: HashMap<String, Money>,
            qbi_deduction_rate: Rate,
            se_tax: SelfEmploymentTaxRules,
            safe_harbor_high_income_threshold: HashMap<String, Money>,
            safe_harbor_high_income_rate: Rate,
        }

        let s = Shadow::deserialize(d)?;

        require_all_filing_statuses(&s.brackets, "federal.brackets")
            .and_then(|_| {
                require_all_filing_statuses(&s.standard_deduction, "federal.standard_deduction")
            })
            .and_then(|_| {
                require_all_filing_statuses(
                    &s.safe_harbor_high_income_threshold,
                    "federal.safe_harbor_high_income_threshold",
                )
            })
            .and_then(|_| {
                require_all_filing_statuses(
                    &s.se_tax.additional_medicare_threshold,
                    "federal.se_tax.additional_medicare_threshold",
                )
            })
            .map_err(serde::de::Error::custom)?;

        Ok(FederalRules {
            brackets: s.brackets,
            standard_deduction: s.standard_deduction,
            qbi_deduction_rate: s.qbi_deduction_rate,
            se_tax: s.se_tax,
            safe_harbor_high_income_threshold: s.safe_harbor_high_income_threshold,
            safe_harbor_high_income_rate: s.safe_harbor_high_income_rate,
        })
    }
}

impl FederalRules {
    pub fn brackets_for(&self, status: FilingStatus) -> Option<&BracketSchedule> {
        self.brackets.get(status.as_key())
    }

    pub fn standard_deduction_for(&self, status: FilingStatus) -> Money {
        self.standard_deduction
            .get(status.as_key())
            .copied()
            .unwrap_or(Money::ZERO)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusinessIncomeDeduction {
    pub exempt_up_to: Money, // e.g. $125,000
    pub rate_above_exemption: Rate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateRules {
    pub brackets: BracketSchedule,
    pub business_income_deduction: Option<BusinessIncomeDeduction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MunicipalRules {
    pub name: String,    // "Toledo"
    pub flat_rate: Rate, // 0.025
}

// ---------------------------------------------------------------------
// The versioned snapshot — this is the whole point: one struct per year,
// loaded from data, never edited in code once shipped
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxRules {
    pub year: TaxYear,
    pub federal: FederalRules,
    /// Keyed by [`StateCode::as_key`].
    pub state: HashMap<String, StateRules>,
    /// Keyed by the [`MunicipalityId`] slug.
    pub municipal: HashMap<String, MunicipalRules>,
}

impl TaxRules {
    pub fn state_for(&self, state: StateCode) -> Option<&StateRules> {
        self.state.get(state.as_key())
    }

    pub fn municipal_for(&self, city: &MunicipalityId) -> Option<&MunicipalRules> {
        self.municipal.get(&city.0)
    }
}
