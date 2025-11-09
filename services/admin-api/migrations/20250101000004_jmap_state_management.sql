-- JMAP State Management Tables
-- Enables delta synchronization and efficient client updates

-- ModSeq tracking per mailbox
CREATE TABLE mailbox_state (
    mailbox_id UUID PRIMARY KEY REFERENCES mailboxes(id) ON DELETE CASCADE,
    modseq BIGINT NOT NULL DEFAULT 0,
    state_token UUID NOT NULL DEFAULT uuid_generate_v4(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_mailbox_state_modseq ON mailbox_state(modseq);

-- Trigger to update mailbox_state timestamp
CREATE OR REPLACE FUNCTION update_mailbox_state_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_mailbox_state_updated
    BEFORE UPDATE ON mailbox_state
    FOR EACH ROW
    EXECUTE FUNCTION update_mailbox_state_timestamp();

-- Change tracking for delta queries (JMAP /get with sinceState)
CREATE TABLE message_state_changes (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    mailbox_id UUID NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    message_id UUID NOT NULL,
    change_type VARCHAR(50) NOT NULL CHECK (change_type IN ('created', 'updated', 'destroyed')),
    modseq BIGINT NOT NULL,
    changed_fields JSONB,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_state_changes_mailbox_modseq ON message_state_changes(mailbox_id, modseq);
CREATE INDEX idx_state_changes_created ON message_state_changes(created_at);

-- Dead letter queue for failed jobs
CREATE TABLE job_dlq (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    queue_name VARCHAR(100) NOT NULL,
    job_data JSONB NOT NULL,
    error_message TEXT,
    retry_count INTEGER NOT NULL DEFAULT 0,
    last_retry_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_job_dlq_queue ON job_dlq(queue_name, created_at);

-- Initialize mailbox_state for all existing mailboxes
INSERT INTO mailbox_state (mailbox_id, modseq, state_token)
SELECT id, 0, uuid_generate_v4()
FROM mailboxes
ON CONFLICT (mailbox_id) DO NOTHING;

-- Function to increment ModSeq and update state token
CREATE OR REPLACE FUNCTION increment_mailbox_modseq(p_mailbox_id UUID)
RETURNS BIGINT AS $$
DECLARE
    v_new_modseq BIGINT;
BEGIN
    UPDATE mailbox_state
    SET
        modseq = modseq + 1,
        state_token = uuid_generate_v4(),
        updated_at = NOW()
    WHERE mailbox_id = p_mailbox_id
    RETURNING modseq INTO v_new_modseq;

    -- If mailbox_state doesn't exist, create it
    IF NOT FOUND THEN
        INSERT INTO mailbox_state (mailbox_id, modseq, state_token)
        VALUES (p_mailbox_id, 1, uuid_generate_v4())
        RETURNING modseq INTO v_new_modseq;
    END IF;

    RETURN v_new_modseq;
END;
$$ LANGUAGE plpgsql;

-- Function to track message state changes
CREATE OR REPLACE FUNCTION track_message_change(
    p_mailbox_id UUID,
    p_message_id UUID,
    p_change_type VARCHAR(50),
    p_changed_fields JSONB DEFAULT NULL
)
RETURNS VOID AS $$
DECLARE
    v_modseq BIGINT;
BEGIN
    -- Get current modseq
    SELECT modseq INTO v_modseq
    FROM mailbox_state
    WHERE mailbox_id = p_mailbox_id;

    -- Insert change record
    INSERT INTO message_state_changes (
        mailbox_id,
        message_id,
        change_type,
        modseq,
        changed_fields
    ) VALUES (
        p_mailbox_id,
        p_message_id,
        p_change_type,
        v_modseq,
        p_changed_fields
    );
END;
$$ LANGUAGE plpgsql;

-- Cleanup old state changes (keep last 30 days)
CREATE OR REPLACE FUNCTION cleanup_old_state_changes()
RETURNS INTEGER AS $$
DECLARE
    v_deleted INTEGER;
BEGIN
    DELETE FROM message_state_changes
    WHERE created_at < NOW() - INTERVAL '30 days';

    GET DIAGNOSTICS v_deleted = ROW_COUNT;
    RETURN v_deleted;
END;
$$ LANGUAGE plpgsql;

-- Trigger to automatically create mailbox_state for new mailboxes
CREATE OR REPLACE FUNCTION create_mailbox_state_on_insert()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO mailbox_state (mailbox_id, modseq, state_token)
    VALUES (NEW.id, 0, uuid_generate_v4())
    ON CONFLICT (mailbox_id) DO NOTHING;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_create_mailbox_state
    AFTER INSERT ON mailboxes
    FOR EACH ROW
    EXECUTE FUNCTION create_mailbox_state_on_insert();
