-- Identifies one incarnation of a user's change sequence (see quest_protocol::Epoch).
ALTER TABLE users ADD COLUMN epoch TEXT NOT NULL DEFAULT '';

UPDATE users SET epoch = lower(
    hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)), 2) || '-a'
    || substr(hex(randomblob(2)), 2) || '-' || hex(randomblob(6))
);
