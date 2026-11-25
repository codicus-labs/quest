-- The data model changed without a migration path (protocol 2): drop every user's synced data.
-- Accounts, OAuth/OIDC data and sign-ins stay. A new epoch invalidates every client cursor;
-- clients speaking protocol 1 are refused, and new clients start from an empty local store, so
-- nothing old is pushed back.
DELETE FROM fields;

UPDATE users SET last_seq = 0, epoch = lower(
    hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)), 2) || '-a'
    || substr(hex(randomblob(2)), 2) || '-' || hex(randomblob(6))
);
