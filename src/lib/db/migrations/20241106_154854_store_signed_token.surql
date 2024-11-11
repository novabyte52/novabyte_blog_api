DEFINE FIELD IF NOT EXISTS signed_token ON nb_token TYPE option<string>;

UPDATE nb_token
SET
    signed_token = NONE;