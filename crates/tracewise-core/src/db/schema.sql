CREATE TABLE files (
                    path TEXT PRIMARY KEY,
                    blob TEXT NOT NULL,
                    size INTEGER NOT NULL,
                    mtime_ns INTEGER NOT NULL
                );
CREATE TABLE changes (
                    id TEXT PRIMARY KEY,
                    start TEXT NOT NULL,
                    end TEXT NOT NULL,
                    path TEXT NOT NULL,
                    op TEXT NOT NULL,
                    old_path TEXT,
                    hunks TEXT NOT NULL,
                    before TEXT,
                    after TEXT,
                    actor_kind TEXT NOT NULL,
                    actor_name TEXT,
                    actor_session TEXT,
                    candidates TEXT NOT NULL,
                    confidence TEXT NOT NULL,
                    source TEXT NOT NULL,
                    command TEXT,
                    intent TEXT,
                    lock_conflict TEXT,
                    device TEXT NOT NULL,
                    branch TEXT NOT NULL,
                    head TEXT NOT NULL,
                    pushed INTEGER NOT NULL DEFAULT 0,
                    prev_hash TEXT
                );
CREATE INDEX idx_changes_path_start ON changes(path, start);
CREATE INDEX idx_changes_actor ON changes(actor_name, start);
CREATE INDEX idx_changes_pushed ON changes(pushed);
CREATE TABLE undo_records (
                    id TEXT PRIMARY KEY,
                    undoes TEXT NOT NULL
                );
CREATE TABLE windows (
                    id TEXT PRIMARY KEY,
                    kind TEXT NOT NULL,
                    actor TEXT NOT NULL,
                    paths TEXT NOT NULL,
                    cwd TEXT,
                    start TEXT NOT NULL,
                    end TEXT,
                    command TEXT
                );
CREATE INDEX idx_windows_open ON windows(end) WHERE end IS NULL;
CREATE TABLE commands (
                    id TEXT PRIMARY KEY,
                    kind TEXT NOT NULL,
                    text TEXT NOT NULL,
                    cwd TEXT,
                    actor TEXT NOT NULL,
                    start TEXT NOT NULL,
                    end TEXT NOT NULL,
                    exit_code INTEGER NOT NULL,
                    changes TEXT NOT NULL DEFAULT '[]'
                );
CREATE INDEX idx_commands_start ON commands(start);
CREATE TABLE intents (
                    id TEXT PRIMARY KEY,
                    actor TEXT NOT NULL,
                    paths TEXT NOT NULL,
                    why TEXT NOT NULL,
                    task TEXT,
                    created TEXT NOT NULL,
                    expires TEXT NOT NULL,
                    kind TEXT NOT NULL,
                    ended INTEGER NOT NULL DEFAULT 0
                );
CREATE INDEX idx_intents_actor ON intents(actor);
CREATE TABLE locks (
                    path TEXT PRIMARY KEY,
                    holder TEXT NOT NULL,
                    since TEXT NOT NULL,
                    expires TEXT NOT NULL,
                    intent TEXT
                , flagged INTEGER NOT NULL DEFAULT 0);
CREATE INDEX idx_locks_expires ON locks(expires);
CREATE TABLE notices (
                    session TEXT PRIMARY KEY,
                    last_prompt_at TEXT NOT NULL
                );
CREATE TABLE shell_state (
                    shell_id TEXT PRIMARY KEY,
                    pending_count INTEGER NOT NULL
                );
CREATE TABLE line_map (
                        path TEXT PRIMARY KEY,
                        runs BLOB NOT NULL
                    );
CREATE TABLE file_changes (
                        path TEXT NOT NULL,
                        change_seq INTEGER NOT NULL,
                        change_id TEXT NOT NULL,
                        PRIMARY KEY (path, change_seq)
                    );
CREATE TABLE file_modes (path TEXT PRIMARY KEY, mode INTEGER NOT NULL);
CREATE INDEX idx_changes_start_id ON changes(start DESC, id DESC);
CREATE TABLE chain_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
