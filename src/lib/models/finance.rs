use serde::{Deserialize, Serialize};
use time::Date;

use super::meta::Meta;
use super::tax_info::{FilingStatus, Money, TaxYear};

// ---------------------------------------------------------------------
// Income
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Payer {
    Employer { name: String },
    Client { name: String },
}

/// How the money is classified for tax purposes — a separate axis from
/// *who* paid it. W-2 wages have SS/Medicare withheld at source; LLC profit
/// carries self-employment tax instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IncomeKind {
    Wage,
    SelfEmploymentProfit,
}

/// Tax already withheld at source. Only meaningful on [`IncomeKind::Wage`]
/// rows — it's the credit that reduces what you still owe each quarter.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Withholding {
    pub federal: Money,
    pub state: Money,
    pub local: Money,
}

impl Withholding {
    pub fn total(&self) -> Money {
        self.federal + self.state + self.local
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomeRecord {
    pub id: String,
    pub person: String,
    pub kind: IncomeKind,
    pub payer: Payer,
    pub amount: Money,
    #[serde(with = "super::date_iso")]
    pub date: Date,
    pub withholding: Option<Withholding>,
    pub category: Option<String>,
    pub note: Option<String>,
    pub meta: Meta<()>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpsertIncomeArgs {
    pub kind: IncomeKind,
    pub payer: Payer,
    pub amount: Money,
    #[serde(with = "super::date_iso")]
    pub date: Date,
    pub withholding: Option<Withholding>,
    pub category: Option<String>,
    pub note: Option<String>,
}

// ---------------------------------------------------------------------
// Expenses
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpenseRecord {
    pub id: String,
    pub person: String,
    pub amount: Money,
    #[serde(with = "super::date_iso")]
    pub date: Date,
    pub category: Option<String>,
    /// Whether this expense reduces net self-employment profit. Non-deductible
    /// expenses are still worth recording for bookkeeping, but they must not
    /// touch the tax calculation.
    pub deductible: bool,
    pub note: Option<String>,
    pub meta: Meta<()>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpsertExpenseArgs {
    pub amount: Money,
    #[serde(with = "super::date_iso")]
    pub date: Date,
    pub category: Option<String>,
    pub deductible: bool,
    pub note: Option<String>,
}

// ---------------------------------------------------------------------
// Estimated payments already made
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Jurisdiction {
    Federal,
    State,
    Municipal,
}

/// The four estimated-tax periods. Note the periods are *unequal* in length
/// (Q2 covers two months, Q4 covers four) even though the standard method
/// requires an equal 25% of the annual liability in each — that asymmetry is
/// why [`Quarter::cumulative_fraction`] and [`Quarter::period_end`] are
/// separate concepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Quarter {
    Q1,
    Q2,
    Q3,
    Q4,
}

impl Quarter {
    pub fn ordinal(&self) -> u8 {
        match self {
            Quarter::Q1 => 1,
            Quarter::Q2 => 2,
            Quarter::Q3 => 3,
            Quarter::Q4 => 4,
        }
    }

    pub fn from_ordinal(n: u8) -> Option<Quarter> {
        match n {
            1 => Some(Quarter::Q1),
            2 => Some(Quarter::Q2),
            3 => Some(Quarter::Q3),
            4 => Some(Quarter::Q4),
            _ => None,
        }
    }

    /// Fraction of the annual liability that must be paid in by the end of
    /// this period under the standard equal-installment method.
    pub fn cumulative_fraction(&self) -> rust_decimal::Decimal {
        rust_decimal::Decimal::from(self.ordinal()) / rust_decimal::Decimal::from(4)
    }

    /// Last day of income covered by this payment period.
    pub fn period_end(&self, year: TaxYear) -> Date {
        let y = year.0 as i32;
        let (month, day) = match self {
            Quarter::Q1 => (time::Month::March, 31),
            Quarter::Q2 => (time::Month::May, 31),
            Quarter::Q3 => (time::Month::August, 31),
            Quarter::Q4 => (time::Month::December, 31),
        };
        Date::from_calendar_date(y, month, day).expect("valid quarter period end")
    }

    /// IRS payment due date for this period. Q4 is due in January of the
    /// *following* year.
    pub fn due_date(&self, year: TaxYear) -> Date {
        let y = year.0 as i32;
        let (yr, month, day) = match self {
            Quarter::Q1 => (y, time::Month::April, 15),
            Quarter::Q2 => (y, time::Month::June, 15),
            Quarter::Q3 => (y, time::Month::September, 15),
            Quarter::Q4 => (y + 1, time::Month::January, 15),
        };
        Date::from_calendar_date(yr, month, day).expect("valid quarter due date")
    }

    /// Days of the tax year elapsed by the end of this payment period —
    /// the denominator when annualizing year-to-date income.
    pub fn days_elapsed(&self, year: TaxYear) -> i64 {
        let start = Date::from_calendar_date(year.0 as i32, time::Month::January, 1)
            .expect("valid year start");
        (self.period_end(year) - start).whole_days() + 1
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxPayment {
    pub id: String,
    pub person: String,
    pub year: TaxYear,
    pub quarter: Quarter,
    pub jurisdiction: Jurisdiction,
    pub amount: Money,
    #[serde(with = "super::date_iso")]
    pub date: Date,
    pub note: Option<String>,
    pub meta: Meta<()>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpsertPaymentArgs {
    pub year: TaxYear,
    pub quarter: Quarter,
    pub jurisdiction: Jurisdiction,
    pub amount: Money,
    #[serde(with = "super::date_iso")]
    pub date: Date,
    pub note: Option<String>,
}

// ---------------------------------------------------------------------
// Read models returned to the UI
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct YtdTotals {
    pub wages: Money,
    pub se_gross: Money,
    pub se_deductible_expenses: Money,
    pub se_net: Money,
    pub non_deductible_expenses: Money,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Projection {
    pub se_tax: Money,
    pub federal_income_tax: Money,
    pub state_income_tax: Money,
    pub municipal_income_tax: Money,
    pub total: Money,
}

/// What one taxing authority is owed for this period.
///
/// Federal, Ohio, and Toledo are three separate payees with three separate
/// credit pools — lumping them into a single number would be convenient and
/// wrong, since W-2 local withholding can't offset a federal shortfall.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JurisdictionDue {
    pub jurisdiction: Jurisdiction,
    pub projected_annual_tax: Money,
    /// Which figure drove `basis_annual`. Safe harbor is federal-only; state
    /// and municipal always report `CurrentYearProjection`.
    pub basis: EstimateBasis,
    pub basis_annual: Money,
    pub required_to_date: Money,
    pub withholding_to_date: Money,
    pub payments_made: Money,
    pub amount_due: Money,
}

/// Which number the quarterly amount was derived from. Safe harbor wins when
/// it's the smaller of the two — paying it protects against the underpayment
/// penalty even if this year's income runs higher than projected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EstimateBasis {
    CurrentYearProjection,
    SafeHarbor,
}

/// How year-to-date income was extrapolated to a full year.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectionMethod {
    /// Scale YTD income by the fraction of the year elapsed. Right when income
    /// arrives steadily.
    Annualized,
    /// Treat what's been received so far as the whole year. Right when the
    /// year's work is already booked, or income is lumpy and front-loaded.
    YtdAsFinal,
}

/// One labeled step of the calculation, in the order it was performed.
/// This is what makes the final number auditable rather than a black box.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EstimateLine {
    pub label: String,
    pub amount: Money,
    pub detail: Option<String>,
}

impl EstimateLine {
    pub fn new(label: impl Into<String>, amount: Money) -> Self {
        Self {
            label: label.into(),
            amount,
            detail: None,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuarterlyEstimate {
    pub year: TaxYear,
    pub quarter: Quarter,
    #[serde(with = "super::date_iso")]
    pub due_date: Date,
    pub filing_status: FilingStatus,
    pub projection_method: ProjectionMethod,
    pub ytd: YtdTotals,
    pub projected_annual_income: YtdTotals,
    pub projected_annual: Projection,
    pub safe_harbor_annual: Option<Money>,
    pub due_by_jurisdiction: Vec<JurisdictionDue>,
    /// The number the button shows: what to send with this quarter's 1040-ES.
    pub federal_amount_due: Money,
    /// Federal + state + municipal, for the headline figure.
    pub amount_due_this_quarter: Money,
    pub lines: Vec<EstimateLine>,
}

/// Lightweight dashboard rollup — no tax rules required, so it still renders
/// for a year that hasn't been configured yet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinanceSummary {
    pub year: TaxYear,
    pub ytd: YtdTotals,
    pub withholding_to_date: Money,
    pub payments_made: Money,
    pub income_count: usize,
    pub expense_count: usize,
    pub has_tax_profile: bool,
    pub has_tax_rules: bool,
}
