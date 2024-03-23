DEFINE TABLE nb_token SCHEMAFULL
    PERMISSIONS
        FOR select, update, delete, create FULL;

DEFINE FIELD person ON nb_token TYPE record<person>;
DEFINE FIELD meta ON nb_token TYPE record<meta>;
