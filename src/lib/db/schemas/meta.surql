DEFINE TABLE meta SCHEMALESS;

DEFINE FIELD created_by ON meta TYPE record<person>;
DEFINE FIELD created_on ON meta TYPE datetime DEFAULT time::now();
DEFINE FIELD modified_by ON meta TYPE option<record<person>> DEFAULT NONE;
DEFINE FIELD modified_on ON meta TYPE option<datetime> DEFAULT NONE;
DEFINE FIELD deleted_by ON meta TYPE option<record<person>> DEFAULT NONE;
DEFINE FIELD deleted_on ON meta TYPE option<datetime> DEFAULT NONE;
DEFINE FIELD data ON meta TYPE option<object> DEFAULT NONE;
