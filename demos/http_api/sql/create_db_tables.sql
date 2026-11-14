CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE DemoItems (
    item_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name TEXT NOT NULL
);

INSERT INTO DemoItems (name) VALUES ('Test item 1');
INSERT INTO DemoItems (name) VALUES ('Test item 2');
INSERT INTO DemoItems (name) VALUES ('Test item 4');
