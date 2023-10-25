import sqlite3
from database_config import DATABASE_PATH

# Connect to the SQLite database
conn = sqlite3.connect(DATABASE_PATH)
cursor = conn.cursor()

# SQL query to retrieve the name of the Ableton Live set based on identifier
live_set_name_sql = """
SELECT name
FROM ableton_live_sets
WHERE identifier = ?;
"""

# The SQL query to retrieve plugins for a specific Ableton Live set
plugin_sql = """
SELECT p.name 
FROM plugins p 
JOIN ableton_live_set_plugins alsp ON p.id = alsp.plugin_id 
JOIN ableton_live_sets als ON als.identifier = alsp.ableton_live_set_id 
WHERE als.identifier = ?;
"""

# The SQL query to retrieve samples for a specific Ableton Live set
sample_sql = """
SELECT s.name 
FROM samples s 
JOIN ableton_live_set_samples alss ON s.id = alss.sample_id 
JOIN ableton_live_sets als ON als.identifier = alss.ableton_live_set_id 
WHERE als.identifier = ?;
"""

# Set a specific identifier for which you want to fetch plugins and samples
identifier_value = 7

try:
    # Fetch and print the Ableton Live set name
    cursor.execute(live_set_name_sql, (identifier_value,))
    live_set_name = cursor.fetchone()
    if live_set_name:
        print(f"Ableton Live Set Name: {live_set_name[0]}")
    else:
        print(f"No Ableton Live Set found for identifier {identifier_value}")

    # Fetch and print the associated plugins
    cursor.execute(plugin_sql, (identifier_value,))
    plugins = cursor.fetchall()
    print("\nPlugins:")
    for plugin in plugins:
        print(plugin[0])

    # Fetch and print the associated samples
    cursor.execute(sample_sql, (identifier_value,))
    samples = cursor.fetchall()
    print("\nSamples:")
    for sample in samples:
        print(sample[0])

except sqlite3.Error as e:
    print(f"SQLite error: {e}")

# Close the database connection
conn.close()
