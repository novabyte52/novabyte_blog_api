DEFINE TABLE IF NOT EXISTS tax_profile SCHEMALESS;

DEFINE FIELD IF NOT EXISTS person ON tax_profile TYPE record<person>;
DEFINE FIELD IF NOT EXISTS year ON tax_profile TYPE int;

DEFINE FIELD IF NOT EXISTS filing_status ON tax_profile TYPE string
    ASSERT $value IN [
        'Single',
        'MarriedFilingJointly',
        'MarriedFilingSeparately',
        'HeadOfHousehold'
    ];

DEFINE FIELD IF NOT EXISTS state ON tax_profile TYPE string;

-- Municipality slug, e.g. 'toledo-oh'. Must match a key in the year's
-- tax_rules.municipal map or the estimate returns an error rather than
-- silently skipping local tax.
DEFINE FIELD IF NOT EXISTS city ON tax_profile TYPE option<string> DEFAULT NONE;

DEFINE FIELD IF NOT EXISTS prefer_itemized ON tax_profile TYPE bool DEFAULT false;
DEFINE FIELD IF NOT EXISTS deductions ON tax_profile TYPE array<object> DEFAULT [];
DEFINE FIELD IF NOT EXISTS prior_year_total_tax ON tax_profile TYPE option<string> DEFAULT NONE;

-- Last year's AGI. What the safe-harbor 110% "high income" multiplier is
-- actually keyed on — deliberately separate from this year's own AGI,
-- which is computed fresh from this year's income at estimate time.
DEFINE FIELD IF NOT EXISTS prior_year_agi ON tax_profile TYPE option<string> DEFAULT NONE;

DEFINE FIELD IF NOT EXISTS meta ON tax_profile TYPE record<meta>;

-- One profile per person per year. Filing status and deductions change from
-- year to year, and past estimates must not shift when this year is edited.
DEFINE INDEX IF NOT EXISTS tax_profile_person_year ON tax_profile FIELDS person, year UNIQUE;
