-- Initialize Nova Mail database for local development

-- This will be run automatically by the postgres container on first startup

\c nova_mail;

-- The main migrations will be applied by sqlx migrate
-- This file just ensures the database is ready

SELECT 'Nova Mail database initialized' AS status;
