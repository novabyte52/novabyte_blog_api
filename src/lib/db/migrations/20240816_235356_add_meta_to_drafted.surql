UPDATE drafted
SET
    meta = (
        SELECT meta
        FROM ONLY post
        WHERE id = $parent.out
        LIMIT 1
    ).meta;