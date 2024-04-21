DEFINE TABLE post SCHEMALESS;

DEFINE FIELD title ON post TYPE string;
DEFINE FIELD markdown ON post TYPE string;
DEFINE FIELD author ON post TYPE record<person>;
DEFINE FIELD meta ON post TYPE record<meta>;
