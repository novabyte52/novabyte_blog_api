DEFINE TABLE IF NOT EXISTS person SCHEMALESS;

DEFINE FIELD IF NOT EXISTS username ON person TYPE string;
DEFINE FIELD IF NOT EXISTS email ON person TYPE string ASSERT string::is::email($value);
DEFINE FIELD IF NOT EXISTS pass_hash ON person TYPE string;
DEFINE FIELD IF NOT EXISTS is_admin ON person TYPE bool DEFAULT false;
DEFINE FIELD IF NOT EXISTS meta ON person TYPE record<meta>;
