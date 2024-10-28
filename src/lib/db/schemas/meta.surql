DEFINE TABLE IF NOT EXISTS meta SCHEMALESS;

DEFINE FIELD IF NOT EXISTS created_by ON meta TYPE record<person>;
DEFINE FIELD IF NOT EXISTS created_on ON meta TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS modified_by ON meta TYPE option<record<person>> DEFAULT NONE;
DEFINE FIELD IF NOT EXISTS modified_on ON meta TYPE option<datetime> DEFAULT NONE;
DEFINE FIELD IF NOT EXISTS deleted_by ON meta TYPE option<record<person>> DEFAULT NONE;
DEFINE FIELD IF NOT EXISTS deleted_on ON meta TYPE option<datetime> DEFAULT NONE;
DEFINE FIELD IF NOT EXISTS data ON meta TYPE option<object> DEFAULT NONE;
