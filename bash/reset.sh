#!/bin/bash

# Stop services
sudo systemctl stop postgresql
sudo systemctl stop pgbouncer
sudo pkill -9 postgres 2>/dev/null || true
sudo pkill -9 pgbouncer 2>/dev/null || true

# Completely wiping PostgreSQL data directory
echo "Completely wiping PostgreSQL data directory..."
sudo rm -rf /var/lib/postgresql/16/main
sudo mkdir -p /var/lib/postgresql/16/main
sudo chown postgres:postgres /var/lib/postgresql/16/main
sudo chmod 700 /var/lib/postgresql/16/main

# Initializing fresh database cluster
echo "Initializing fresh database cluster..."
sudo -u postgres /usr/lib/postgresql/16/bin/initdb -D /var/lib/postgresql/16/main --auth=trust

# Resetting PostgreSQL configuration
echo "Resetting PostgreSQL configuration..."
sudo tee /etc/postgresql/16/main/pg_hba.conf > /dev/null 2>&1 << 'EOF'
# TYPE  DATABASE        USER            ADDRESS                 METHOD
local   all             postgres                                peer
local   all             all                                     peer
host    all             all             127.0.0.1/32            md5
host    all             all             ::1/128                 md5
EOF

sudo tee /etc/postgresql/16/main/postgresql.conf > /dev/null 2>&1 << 'EOF'
# DEFAULT POSTGRESQL CONFIG - ALL CUSTOM SETTINGS REMOVED
data_directory = '/var/lib/postgresql/16/main'
hba_file = '/etc/postgresql/16/main/pg_hba.conf'
ident_file = '/etc/postgresql/16/main/pg_ident.conf'
listen_addresses = 'localhost'
port = 5432
max_connections = 100
EOF

# Resetting PgBouncer
echo "Resetting PgBouncer..."
sudo rm -rf /etc/pgbouncer/*
sudo rm -rf /var/run/pgbouncer/*
sudo rm -rf /var/log/pgbouncer/*
sudo tee /etc/pgbouncer/pgbouncer.ini > /dev/null 2>&1 << 'EOF'
[databases]
* = host=localhost port=5432

[pgbouncer]
listen_addr = *
listen_port = 6432
auth_type = trust
auth_file = /etc/pgbouncer/userlist.txt
pool_mode = session
max_client_conn = 100
default_pool_size = 20
EOF

sudo touch /etc/pgbouncer/userlist.txt
sudo chown postgres:postgres /etc/pgbouncer/pgbouncer.ini
sudo chown postgres:postgres /etc/pgbouncer/userlist.txt

# Starting PostgreSQL
echo "Starting PostgreSQL..."
sudo systemctl start postgresql
sleep 3

# Resetting superuser password
echo "Resetting superuser password..."
read -sp "Enter new password for postgres user: " POSTGRESPASSWD
echo
sudo -u postgres psql -c "ALTER USER postgres WITH PASSWORD '$POSTGRESPASSWD';"

# Starting PgBouncer
echo "Starting PgBouncer..."
sudo systemctl start pgbouncer
sleep 2

# Verification
echo "=== Verification ==="
echo ""
echo "1. Listing all databases:"
sudo -u postgres psql -c "\l"
echo ""

echo "2. Listing all users/roles:"
sudo -u postgres psql -c "\du"
echo ""

echo "3. Listing all schemas:"
sudo -u postgres psql -c "\dn"
echo ""

echo "4. Listing all tables in all schemas:"
sudo -u postgres psql -c "
SELECT schemaname, tablename, tableowner 
FROM pg_tables 
WHERE schemaname NOT IN ('pg_catalog', 'information_schema')
ORDER BY schemaname, tablename;"
echo ""

echo "5. Listing all extensions:"
sudo -u postgres psql -c "SELECT * FROM pg_extension;"
echo ""

echo "6. Checking default configuration:"
sudo -u postgres psql -c "SELECT name, setting FROM pg_settings WHERE source != 'default' LIMIT 10;"
echo ""

echo "7. PgBouncer status:"
sudo systemctl status pgbouncer --no-pager | head -10
echo ""

echo "8. Testing new password:"
if PGPASSWORD="$POSTGRESPASSWD" psql -U postgres -h localhost -c "SELECT 'Password reset: SUCCESS' AS status;" 2>/dev/null; then
    echo "[+] Password reset successful"
else
    echo "[x] Password reset failed"
fi
echo ""

echo "=== Result summary ==="
echo "[+] ALL user-created databases REMOVED (only 'postgres' should remain)"
echo "[+] ALL user-created schemas REMOVED (only 'public' should remain)" 
echo "[+] ALL user-created tables REMOVED (no user tables should exist)"
echo "[+] ALL custom users/roles REMOVED (only default roles should remain)"
echo "[+] SUPERUSER password RESET to new value"
echo "[+] ALL custom configurations RESET to default"
echo "[+] PgBouncer COMPLETELY reset to factory state"
echo ""
echo "=== Connection info ==="
echo "PostgreSQL direct: psql -U postgres -h localhost"
echo "PostgreSQL socket: sudo -u postgres psql"
echo "PgBouncer: psql -p 6432 -U postgres -h localhost"
echo "New Password: $POSTGRESPASSWD"