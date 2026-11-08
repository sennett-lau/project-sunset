-- Crash recovery for a file renamed before its manifest acknowledgement.
ALTER TABLE vault_files ADD COLUMN pending_content TEXT;
