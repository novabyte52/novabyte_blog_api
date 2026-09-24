DEFINE TABLE IF NOT EXISTS tax_payment SCHEMALESS;

DEFINE FIELD IF NOT EXISTS person ON tax_payment TYPE record<person>;
DEFINE FIELD IF NOT EXISTS year ON tax_payment TYPE int;

DEFINE FIELD IF NOT EXISTS quarter ON tax_payment TYPE string
    ASSERT $value IN ['Q1', 'Q2', 'Q3', 'Q4'];

DEFINE FIELD IF NOT EXISTS jurisdiction ON tax_payment TYPE string
    ASSERT $value IN ['Federal', 'State', 'Municipal'];

DEFINE FIELD IF NOT EXISTS amount ON tax_payment TYPE string;
DEFINE FIELD IF NOT EXISTS date ON tax_payment TYPE string;
DEFINE FIELD IF NOT EXISTS note ON tax_payment TYPE option<string> DEFAULT NONE;
DEFINE FIELD IF NOT EXISTS meta ON tax_payment TYPE record<meta>;

DEFINE INDEX IF NOT EXISTS payment_person_year ON tax_payment FIELDS person, year;
