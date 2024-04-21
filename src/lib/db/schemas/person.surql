DEFINE TABLE person SCHEMALESS;

DEFINE FIELD username ON person TYPE string;
DEFINE FIELD email ON person TYPE string ASSERT string::is::email($value);
DEFINE FIELD pass_hash ON person TYPE string;
DEFINE FIELD is_admin ON person TYPE bool DEFAULT false;
DEFINE FIELD meta ON person TYPE record<meta>;
