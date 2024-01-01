DEFINE TABLE person SCHEMAFULL
    PERMISSIONS
        FOR select FULL
        FOR update WHERE id = $auth.id
        FOR delete WHERE id = $auth.id
        FOR create FULL;

DEFINE FIELD username ON person TYPE string;
DEFINE FIELD email ON person TYPE string ASSERT string::is::email($value);
DEFINE FIELD meta ON person TYPE record<meta>;
