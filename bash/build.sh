#!/usr/bin/env bash

# Define PGVER
PGVER="16"

# Define PGETC
PGETC="/etc/postgresql/$PGVER/main"

# Define PGDATA
PGDATA="/var/lib/postgresql/$PGVER/main"

# Define PGRUN
PGRUN="/var/run/postgresql"

# Define PGLOG
PGLOG="/var/log/postgresql"

# Define PGCONF
PGCONF="$PGETC/postgresql.conf"

# Define PGHBA
PGHBA="$PGETC/pg_hba.conf"

# Define PIDFILE
PIDFILE="$PGRUN/$PGVER-main.pid"

# Define PGBETC
PGBETC="/etc/pgbouncer"

# Define PGBINI
PGBINI="$PGBETC/pgbouncer.ini"

# Define PGBUSERLIST
PGBUSERLIST="$PGBETC/userlist.txt"

# Function getappconfig()
getappconfig()
{
    local __var="$1" __label="$2" val
    local re='^[A-Za-z_][A-Za-z0-9_]{0,62}$'
    while :;
    do
        read -r -p "$__label: " val
        if [[ -z "$val" ]];
        then
            echo "Value cannot be empty.";
            continue;
        fi

        if [[ ! "$val" =~ $re ]];
        then
            echo "Must start with letter/_ and max 63 chars.";
            continue
        fi

        printf -v "$__var" '%s' "$val"; export "$__var";
        break

    done
}

# Function getpasswd()
getpasswd()
{
    local __var="$1" __label="$2" p1 p2
    while :;
    do
        read -rs -p "$__label: " p1; echo
        local cleanlabel="${__label#Enter }"
        read -rs -p "Confirm $cleanlabel: " p2; echo
        if [[ -z "$p1" ]];
        then
            echo "Password cannot be empty.";
            continue;
        fi

        if [[ "${#p1}" -lt 8 ]];
        then
            echo "Password must be at least 8 chars.";
            continue;
        fi

        if [[ "$p1" != "$p2" ]];
        then
            echo "Passwords do not match.";
            continue;
        fi

        printf -v "$__var" '%s' "$p1"; export "$__var";
        break
    done
}

# Function rs()
ts()
{
    date +%Y%m%d-%H%M%S;
}

# Function says()
say()
{
    echo -e "\033[1;36m>> $*\033[0m";
}

# Function pgconf()
pgconf()
{
    sudo pg_conftool -- "$PGVER" main set "$1" "$2";
}

# Collect inputs
getappconfig APPUSER "Enter APP username"
getappconfig APPDB "Enter DB name"
getpasswd APPPASS "Enter APP user password"
getpasswd PGPASSWD "Enter PgBouncer user password"

# Callback says()
say "Using PG_VER=$PGVER, APP_DB=$APPDB, APP_USER=$APPUSER"

# Install packages
if ! command -v psql >/dev/null 2>&1;
then
    sudo apt-get update -y
    sudo apt-get install -y "postgresql-$PGVER" "postgresql-client-$PGVER" postgresql-common
fi

if ! command -v pg_conftool >/dev/null 2>&1;
then
    sudo apt-get install -y postgresql-common
fi

if ! command -v pgbouncer >/dev/null 2>&1;
then
    sudo apt-get install -y pgbouncer
fi

# Ensure cluster exists
if command -v pg_lsclusters >/dev/null 2>&1 && pg_lsclusters | awk '{print $1,$2}' | grep -q "^$PGVER main$";
then
    if [ ! -f "$PGDATA/PG_VERSION" ];
    then
        say "Cluster entry exists but data dir missing; recreating…"
        sudo systemctl stop postgresql || true
        sudo pg_dropcluster --stop "$PGVER" main || true
        sudo rm -rf "$PGDATA" "$PGETC" || true
        sudo pg_createcluster --start "$PGVER" main
    else
        sudo systemctl restart postgresql
    fi
else
    say "Creating cluster $PGVER/main…"
    sudo pg_createcluster --start "$PGVER" main
fi

# Create Postgres directory
sudo mkdir -p "$PGRUN" "$PGLOG"
sudo chown -R postgres:postgres "$PGRUN" "$PGLOG"

# Backups (non-destructive)
[ -f "$PGCONF" ] && sudo cp -a "$PGCONF" "$PGCONF.bak.$(ts)" || true
[ -f "$PGHBA" ]  && sudo cp -a "$PGHBA"  "$PGHBA.bak.$(ts)"  || true
[ -f "$PGBINI" ] && sudo cp -a "$PGBINI" "$PGBINI.bak.$(ts)" || true
[ -f "$PGBUSERLIST" ] && sudo cp -a "$PGBUSERLIST" "$PGBUSERLIST.bak.$(ts)" || true

# Tune Postgres via pg_conftool
pgconf data_directory "$PGDATA"
pgconf hba_file "$PGHBA"
pgconf external_pid_file "$PIDFILE"

pgconf listen_addresses '*'
pgconf port '5432'
pgconf max_connections '200'
pgconf superuser_reserved_connections '3'
pgconf unix_socket_directories "$PGRUN"

pgconf ssl 'on'
pgconf ssl_cert_file '/etc/ssl/certs/ssl-cert-snakeoil.pem'
pgconf ssl_key_file  '/etc/ssl/private/ssl-cert-snakeoil.key'

# Memory (fallbacks if host is small)
pgconf shared_buffers '64GB' || pgconf shared_buffers '4GB'
pgconf huge_pages 'try'
pgconf work_mem '64MB'
pgconf maintenance_work_mem '4GB'
pgconf effective_cache_size '384GB' || pgconf effective_cache_size '12GB'
pgconf dynamic_shared_memory_type 'posix'

# Writer & IO
pgconf bgwriter_delay '200ms'
pgconf bgwriter_lru_maxpages '1000'
pgconf bgwriter_lru_multiplier '2.0'
pgconf bgwriter_flush_after '512kB'
pgconf effective_io_concurrency '256'
pgconf maintenance_io_concurrency '64'

# Parallelism
pgconf max_worker_processes '32'
pgconf max_parallel_workers '32'
pgconf max_parallel_workers_per_gather '16'
pgconf max_parallel_maintenance_workers '2'
pgconf parallel_leader_participation 'on'

# WAL
pgconf wal_level 'replica'
pgconf synchronous_commit 'off'
pgconf full_page_writes 'on'
pgconf wal_compression 'on'
pgconf wal_buffers '-1'
pgconf wal_writer_delay '200ms'
pgconf wal_writer_flush_after '1MB'
pgconf checkpoint_timeout '30min'
pgconf checkpoint_completion_target '0.9'
pgconf checkpoint_flush_after '256kB'
pgconf checkpoint_warning '30s'
pgconf max_wal_size '16GB'
pgconf min_wal_size '2GB'

# Planner tweaks
pgconf random_page_cost '1.1'
pgconf cpu_tuple_cost '0.01'
pgconf cpu_index_tuple_cost '0.005'
pgconf cpu_operator_cost '0.0025'

# Logging
pgconf log_destination 'stderr'
pgconf logging_collector 'on'
pgconf log_directory 'log'
pgconf log_filename 'postgresql-%a.log'
pgconf log_truncate_on_rotation 'on'
pgconf log_rotation_age '1d'
pgconf log_rotation_size '0'
pgconf log_min_duration_statement '500ms'
pgconf log_checkpoints 'on'
pgconf log_autovacuum_min_duration '1s'
pgconf log_line_prefix '%m [%p] %q%u@%d '
pgconf log_timezone 'America/Bogota'

# Stats & extensions
pgconf track_activities 'on'
pgconf track_counts 'on'
pgconf track_io_timing 'on'
pgconf shared_preload_libraries ''

# Autovacuum
pgconf autovacuum 'on'
pgconf autovacuum_max_workers '6'
pgconf autovacuum_naptime '5s'
pgconf autovacuum_vacuum_threshold '10000'
pgconf autovacuum_vacuum_insert_threshold '1000'
pgconf autovacuum_analyze_threshold '5000'
pgconf autovacuum_vacuum_scale_factor '0.10'
pgconf autovacuum_vacuum_insert_scale_factor '0.20'
pgconf autovacuum_analyze_scale_factor '0.05'
pgconf autovacuum_freeze_max_age '200000000'
pgconf autovacuum_multixact_freeze_max_age '400000000'
pgconf autovacuum_vacuum_cost_limit '8000'

# Client defaults
pgconf datestyle 'iso, mdy'
pgconf timezone 'America/Bogota'
pgconf statement_timeout '0'
pgconf lock_timeout '0'
pgconf idle_in_transaction_session_timeout '0'
pgconf idle_session_timeout '0'
pgconf deadlock_timeout '1s'
pgconf include_dir 'conf.d'

# Enforce SCRAM on loopback
if ! sudo grep -qE '^\s*host\s+all\s+all\s+127\.0\.0\.1/32\s+scram-sha-256' "$PGHBA";
then
    echo "host all all 127.0.0.1/32 scram-sha-256" | sudo tee -a "$PGHBA" >/dev/null
fi

# Restart Postgres
sudo systemctl restart postgresql

# Define roles/db/lookup function
sudo -u postgres psql -v ON_ERROR_STOP=1 postgres <<SQL
DO \$\$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = '$APPUSER') THEN
        EXECUTE 'ALTER ROLE ' || quote_ident('$APPUSER') || ' LOGIN PASSWORD ' || quote_literal('$APPPASS');
    ELSE
        EXECUTE 'CREATE ROLE ' || quote_ident('$APPUSER') || ' LOGIN PASSWORD ' || quote_literal('$APPPASS');
    END IF;

    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'pgbouncer') THEN
        EXECUTE 'ALTER ROLE pgbouncer LOGIN PASSWORD ' || quote_literal('$PGPASSWD');
    ELSE
        EXECUTE 'CREATE ROLE pgbouncer LOGIN PASSWORD ' || quote_literal('$PGPASSWD');
    END IF;
END
\$\$;

SELECT 'CREATE DATABASE ' || quote_ident('$APPDB') || ' OWNER ' || quote_ident('$APPUSER')
WHERE NOT EXISTS (SELECT 1 FROM pg_database WHERE datname = '$APPDB') \gexec

-- Auth objects live in the *postgres* database:
CREATE SCHEMA IF NOT EXISTS pgbouncer AUTHORIZATION postgres;

CREATE OR REPLACE FUNCTION pgbouncer.lookup_user(wanted text)
RETURNS TABLE(username text, passwd text)
LANGUAGE sql
SECURITY DEFINER
SET search_path = pg_catalog
AS \$\$
    SELECT u.rolname::text, u.rolpassword::text
    FROM pg_authid u
    WHERE u.rolcanlogin AND u.rolname = wanted
\$\$;

ALTER FUNCTION pgbouncer.lookup_user(text) OWNER TO postgres;

-- Critical grants
REVOKE ALL ON SCHEMA pgbouncer FROM PUBLIC;
GRANT USAGE ON SCHEMA pgbouncer TO pgbouncer;   -- needed
REVOKE ALL ON FUNCTION pgbouncer.lookup_user(text) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION pgbouncer.lookup_user(text) TO pgbouncer;

GRANT CONNECT ON DATABASE "$APPDB" TO "$APPUSER";

-- Self-check (returns APP_USER + SCRAM hash)
SELECT * FROM pgbouncer.lookup_user('$APPUSER');
SQL

# Create PgBouncer directory
sudo mkdir -p "$PGBETC"

# Create PgBouncer password config
sudo tee "$PGBUSERLIST" >/dev/null <<EOF
"pgbouncer" "$PGPASSWD"
EOF

sudo chmod 600 "$PGBUSERLIST"
if id pgbouncer &>/dev/null;
then
    sudo chown pgbouncer:pgbouncer "$PGBUSERLIST";
else
    sudo chown postgres:postgres "$PGBUSERLIST";
fi

# Create PgBouncer init file
sudo tee "$PGBINI" >/dev/null <<EOF
[databases]
$APPDB  = host=127.0.0.1 port=5432 dbname=$APPDB
postgres = host=127.0.0.1 port=5432 dbname=postgres

[pgbouncer]
listen_addr = 127.0.0.1
listen_port = 6432
unix_socket_dir = /run/pgbouncer

auth_type = scram-sha-256
auth_user = pgbouncer
auth_dbname = postgres
auth_query = SELECT username, passwd FROM pgbouncer.lookup_user(\$1)
auth_file = $PGBUSERLIST

pool_mode = session
default_pool_size = 120
min_pool_size = 30
reserve_pool_size = 30
reserve_pool_timeout = 2.0
max_client_conn = 800
query_wait_timeout = 3.0
query_timeout = 6.0
server_reset_query = DISCARD ALL
server_round_robin = 1

admin_users = postgres, pgbouncer
stats_users = postgres, pgbouncer

ignore_startup_parameters = extra_float_digits, options, search_path, application_name

logfile =
syslog = 1
pidfile = /run/pgbouncer/pgbouncer.pid
EOF

if id pgbouncer &>/dev/null;
then
    sudo chown pgbouncer:pgbouncer "$PGBINI";
else
    sudo chown postgres:postgres "$PGBINI";
fi

sudo chmod 640 "$PGBINI"

# Edit PgBouncer override
sudo mkdir -p /etc/systemd/system/pgbouncer.service.d
sudo tee /etc/systemd/system/pgbouncer.service.d/override.conf >/dev/null <<'OVR'
[Service]
RuntimeDirectory=pgbouncer
RuntimeDirectoryMode=0755
LimitNOFILE=131072
OVR

# Restart PgBouncer
sudo systemctl daemon-reload
sudo systemctl restart postgresql
sudo systemctl restart pgbouncer

# Smoke tests
say "Test: direct to Postgres (as app user)"
psql "postgres://$APPUSER:$APPPASS@127.0.0.1:5432/$APPDB" -c "SELECT 1;"

say "Test: lookup function (as pgbouncer) against postgres db"
psql "postgres://pgbouncer:$PGPASSWD@127.0.0.1:5432/postgres" -c "SELECT * FROM pgbouncer.lookup_user('$APPUSER');"

say "Test: via PgBouncer (as app user)"
psql "postgres://$APPUSER:$APPPASS@127.0.0.1:6432/$APPDB" -c "SELECT 1;" || {
  echo "PgBouncer test failed — check: journalctl -xeu pgbouncer.service" >&2
  exit 1
}

say "All good"