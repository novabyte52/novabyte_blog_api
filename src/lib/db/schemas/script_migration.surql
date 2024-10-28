DEFINE TABLE IF NOT EXISTS script_migration SCHEMAFULL
    PERMISSIONS
        FOR select FULL
        FOR create, update, delete NONE;

DEFINE FIELD IF NOT EXISTS script_name ON script_migration TYPE string;
DEFINE FIELD IF NOT EXISTS executed_at ON script_migration TYPE datetime DEFAULT time::now();
