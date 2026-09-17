DEFINE TABLE IF NOT EXISTS tax_rules SCHEMALESS;

DEFINE FIELD IF NOT EXISTS year ON tax_rules TYPE int;

-- The whole TaxRules snapshot as a nested object: federal brackets, standard
-- deductions, SE tax rates, and the per-state / per-municipality tables.
-- Kept as one blob because it is always read and written whole, and because
-- the shape is versioned by year rather than normalized.
DEFINE FIELD IF NOT EXISTS rules ON tax_rules TYPE object;

DEFINE FIELD IF NOT EXISTS meta ON tax_rules TYPE record<meta>;

-- Rules describe the law, not a person, so they are shared across accounts.
DEFINE INDEX IF NOT EXISTS tax_rules_year ON tax_rules FIELDS year UNIQUE;
