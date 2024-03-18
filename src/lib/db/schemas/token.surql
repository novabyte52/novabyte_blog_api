DEFINE TABLE nb_token SCHEMAFULL
    PERMISSIONS
        FOR select, update, delete, create FULL;

DEFINE FIELD username ON nb_token TYPE string;
DEFINE FIELD email ON nb_token TYPE string ASSERT string::is::email($value);
DEFINE FIELD pass_hash ON nb_token TYPE string;
DEFINE FIELD meta ON nb_token TYPE record<meta>;
