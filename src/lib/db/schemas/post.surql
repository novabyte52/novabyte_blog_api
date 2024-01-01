DEFINE TABLE post SCHEMAFULL
    PERMISSIONS
        FOR select FULL
        FOR create WHERE permission:create_post IN $auth.permissions
        FOR update, delete WHERE author = $auth.id;

DEFINE FIELD title ON post TYPE string;
DEFINE FIELD markdown ON post TYPE string;
DEFINE FIELD author ON post TYPE record<person>;
DEFINE FIELD meta ON post TYPE record<meta>;
