DEFINE FUNCTION fn::string_id($record_id: record) {
    string::concat(meta::tb($record_id) + ':', meta::id($record_id))
};
