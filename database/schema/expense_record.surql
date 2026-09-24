DEFINE TABLE IF NOT EXISTS expense_record SCHEMALESS;

DEFINE FIELD IF NOT EXISTS person ON expense_record TYPE record<person>;
DEFINE FIELD IF NOT EXISTS amount ON expense_record TYPE string;
DEFINE FIELD IF NOT EXISTS date ON expense_record TYPE string;
DEFINE FIELD IF NOT EXISTS category ON expense_record TYPE option<string> DEFAULT NONE;

-- Only deductible expenses reduce net self-employment profit. Non-deductible
-- rows are kept for bookkeeping but must never touch the tax calculation.
DEFINE FIELD IF NOT EXISTS deductible ON expense_record TYPE bool DEFAULT true;

DEFINE FIELD IF NOT EXISTS note ON expense_record TYPE option<string> DEFAULT NONE;
DEFINE FIELD IF NOT EXISTS meta ON expense_record TYPE record<meta>;

DEFINE INDEX IF NOT EXISTS expense_person_date ON expense_record FIELDS person, date;
