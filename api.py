from flask import Flask, jsonify, make_response
from contextlib import closing
import sqlite3
from database_config import DATABASE_PATH
from logging_utility import log


app = Flask(__name__)

@app.route('/api/ableton_live_sets', methods=['GET'])
def get_ableton_live_sets():
    try:
        with closing(sqlite3.connect(DATABASE_PATH)) as conn, closing(conn.cursor()) as cursor:
            cursor.execute(
                ("SELECT identifier, path, file_hash, last_scan_timestamp, "
                "name, creation_time, last_modification_time, "
                "creator, key, tempo, time_signature, estimated_duration "
                "FROM ableton_live_sets")
            )

            live_sets = cursor.fetchall()
            output = []

            for live_set in live_sets:
                log.info(f"Processing live_set: {live_set}")
                
                data = {
                    'identifier': live_set[0],
                    'path': live_set[1],
                    'file_hash': live_set[2],
                    'last_scan_timestamp': live_set[3],
                    'name': live_set[4],
                    'creation_time': live_set[5],
                    'last_modification_time': live_set[6],
                    'creator': live_set[7],
                    'key': live_set[8],
                    'tempo': live_set[9],
                    'time_signature': live_set[10],
                    'estimated_duration': live_set[11],
                    'plugins': [],
                    'samples': []
                }

                cursor.execute("SELECT plugins.id, plugins.name FROM plugins "
                            "INNER JOIN ableton_live_set_plugins ON plugins.id = ableton_live_set_plugins.plugin_id "
                            "WHERE ableton_live_set_plugins.ableton_live_set_id = ?", (live_set[0],))
                plugins = cursor.fetchall()

                log.info(f"plugins found: {plugins}")

                for plugin in plugins:
                    plugin_data = {
                        'id': plugin[0],
                        'name': plugin[1]
                    }
                    data['plugins'].append(plugin_data)

                cursor.execute("SELECT samples.id, samples.name FROM samples "
                            "INNER JOIN ableton_live_set_samples ON samples.id = ableton_live_set_samples.sample_id "
                            "WHERE ableton_live_set_samples.ableton_live_set_id = ?", (live_set[1],))
                samples = cursor.fetchall()

                log.info(f"samples found: {samples}")

                for sample in samples:
                    sample_data = {
                        'id': sample[0],
                        'name': sample[1]
                    }
                    data['samples'].append(sample_data)
                log.info(f"Final data object for current live_set: {data}")
                output.append(data)

            return make_response(jsonify({'ableton_live_sets': output}), 200)
        
    except sqlite3.Error as e:
        # Log error and return an error code
        log.error(f"Database error: {e}")
        return make_response(jsonify({'error': 'Database error'}), 500)


if __name__ == '__main__':
    app.run(debug=True, port=5000)