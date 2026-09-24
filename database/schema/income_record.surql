DEFINE TABLE IF NOT EXISTS income_record SCHEMALESS;

DEFINE FIELD IF NOT EXISTS person ON income_record TYPE record<person>;

-- 'Wage' | 'SelfEmploymentProfit'
DEFINE FIELD IF NOT EXISTS kind ON income_record TYPE string
    ASSERT $value IN ['Wage', 'SelfEmploymentProfit'];

-- Externally tagged enum: { Employer: { name } } | { Client: { name } }
DEFINE FIELD IF NOT EXISTS payer ON income_record TYPE object;

-- Money is stored as a decimal string, never a float: the API round-trips
-- every value through JSON, and a JSON number would land as an f64 and lose
-- cents. Same for every other amount in these tables.
DEFINE FIELD IF NOT EXISTS amount ON income_record TYPE string;

-- ISO 'YYYY-MM-DD'. Stored as a string so year filtering is a prefix match
-- and the values still sort correctly as text.
DEFINE FIELD IF NOT EXISTS date ON income_record TYPE string;

DEFINE FIELD IF NOT EXISTS withholding ON income_record TYPE option<object> DEFAULT NONE;
DEFINE FIELD IF NOT EXISTS withholding.federal ON income_record TYPE string;
DEFINE FIELD IF NOT EXISTS withholding.state ON income_record TYPE string;
DEFINE FIELD IF NOT EXISTS withholding.local ON income_record TYPE string;

DEFINE FIELD IF NOT EXISTS category ON income_record TYPE option<string> DEFAULT NONE;
DEFINE FIELD IF NOT EXISTS note ON income_record TYPE option<string> DEFAULT NONE;
DEFINE FIELD IF NOT EXISTS meta ON income_record TYPE record<meta>;

DEFINE INDEX IF NOT EXISTS income_person_date ON income_record FIELDS person, date;
