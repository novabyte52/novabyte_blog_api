DEFINE TABLE IF NOT EXISTS nb_token SCHEMALESS;

DEFINE FIELD IF NOT EXISTS person ON nb_token TYPE record<person>;
DEFINE FIELD IF NOT EXISTS meta ON nb_token TYPE record<meta>;
