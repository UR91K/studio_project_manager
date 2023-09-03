"""
A GUI Application for Ableton Live Set Management

This module provides a GUI application using PyQt5 to allow users to perform a full-text search on
Ableton Live Set projects stored in a SQLite database. Search results include key project details such 
as name, creator, key, tempo, time signature, estimated duration, furthest bar, plugins used, and sample paths.

The app provides a search bar at the top where users can enter their query. Search results are displayed 
in a table format with columns representing different attributes of the Ableton Live Set project.

Attributes:
    DATABASE_PATH (Path): Path to the SQLite database file.
    columns_info_dict (dict): Dictionary containing column names and their corresponding indexes.
    excluded_columns (list): List of column names to be excluded from the display.

Functions:
    perform_full_text_search(query): Executes a full-text search on the SQLite database using the provided query 
                                     and returns the results.

Classes:
    SearchApp(QMainWindow): Main GUI class responsible for displaying the search bar, handling input,
                            and displaying search results.

Note:
    This module initializes the SQLite database with the required tables and columns if they do not exist 
    and loads necessary configurations from a "config.toml" file. The app currently focuses on GUI and search 
    functionalities, with plans for additional features and improvements in the future.
"""

import os
import sqlite3
import sys
from pathlib import Path

import toml
from fuzzywuzzy import fuzz
from PyQt5.QtCore import QTimer
from PyQt5.QtWidgets import (
    QApplication,
    QComboBox,
    QLineEdit,
    QMainWindow,
    QTableWidget,
    QTableWidgetItem,
    QVBoxLayout,
    QWidget,
)

os.chdir(os.path.dirname(os.path.abspath(__file__)))
config = toml.load("config.toml")
user_home_dir = os.path.expanduser("~")
DATABASE_PATH = Path(config["database_path"]["path"].replace("{USER_HOME}", user_home_dir))
SEARCH_RESULT_COUNT = 20
print("Database path: ", DATABASE_PATH)

conn = sqlite3.connect(DATABASE_PATH)
cursor = conn.cursor()
cursor.execute("PRAGMA table_info(ableton_live_sets)")
columns_info = cursor.fetchall()
columns_info_dict = {info[1]: index for index, info in enumerate(columns_info)}

excluded_columns = ["uuid", 
                    "identifier", 
                    "xml_root", 
                    "path", 
                    "file_hash", 
                    "last_scan_timestamp", 
                    "major_version", 
                    "minor_version", 
                    "creator",
                    "furthest_bar"
                    ]

HEADER_MAPPING = {
    'name': 'Name',
    'creation_time': 'Creation Time',
    'last_modification_time': 'Last Modified',
    'key': 'Key',
    'major_minor_patch': 'Version',
    'tempo': 'Tempo',
    'time_signature': 'Time Signature',
    'estimated_duration': 'Estimated Duration',
    'furthest_bar': 'Furthest Bar',
    'plugins': 'Plugins',
    'samples': 'Samples'
}

def get_best_match_score(row, query, exclude_columns):
    best_score = 0
    for index, column_info in enumerate(columns_info):
        column_name = column_info[1]
        if column_name not in exclude_columns and row[index] is not None:
            score = fuzz.ratio(str(row[index]).lower(), query.lower())
            best_score = max(best_score, score)
    return best_score

class MainWindow(QMainWindow):
    def __init__(self):
        super().__init__()

        self.init_ui()

    def init_ui(self):
        layout = QVBoxLayout()

        self.search_bar = QLineEdit(self)
        self.search_bar.textChanged.connect(self.on_search_change)
        layout.addWidget(self.search_bar)

        self.results_table = QTableWidget(self)
        self.headers = [HEADER_MAPPING.get(info[1], info[1]) for info in columns_info if info[1] not in excluded_columns]
        self.results_table.setHorizontalHeaderLabels(self.headers)
        layout.addWidget(self.results_table)

        central_widget = QWidget()
        central_widget.setLayout(layout)
        self.setCentralWidget(central_widget)

        self.debounce_timer = QTimer(self)
        self.debounce_timer.setSingleShot(True)
        self.debounce_timer.timeout.connect(self.search_database)

        self.headers = [info[1] for info in columns_info if info[1] not in excluded_columns]
        if 'Plugins' not in self.headers:
            self.headers.append('Plugins')
        if 'Samples' not in self.headers:
            self.headers.append('Samples')
        self.friendly_headers = [HEADER_MAPPING.get(header, header) for header in self.headers]
        self.friendly_headers.extend(['Plugins', 'Samples'])
        self.results_table.setHorizontalHeaderLabels(self.friendly_headers)

    def on_search_change(self):
        self.debounce_timer.stop()
        self.debounce_timer.start(300)

    def search_database(self):
        query = self.search_bar.text()

        cursor.execute("SELECT * FROM ableton_live_sets")
        rows = cursor.fetchall()



        if not query:
            # If the query is empty, consider all rows as matching
            matching_rows = rows
        else:
            exclude_columns = ["uuid", "identifier", "xml_root", "path", "file_hash", "last_scan_timestamp"]
            
            # Get the best match score for each row
            match_scores = [(row, get_best_match_score(row, query, exclude_columns)) for row in rows]

            # Sort rows based on score and take the top results (you can adjust the number)
            sorted_matches = sorted(match_scores, key=lambda x: x[1], reverse=True)
            matching_rows = [match[0] for match in sorted_matches[:SEARCH_RESULT_COUNT]]

        # Populate the results table with matching_rows
        self.results_table.setRowCount(len(matching_rows))
        self.results_table.setColumnCount(len(self.headers))

        for row_idx, row in enumerate(matching_rows):
            for col_idx, col in enumerate(row):
                col_name = columns_info[col_idx][1]
                if col_name not in excluded_columns:
                    self.results_table.setItem(row_idx, self.headers.index(col_name), QTableWidgetItem(str(col)))

            ableton_set_id = row[columns_info_dict["identifier"]]

            cursor.execute("""
                SELECT name FROM plugins
                JOIN ableton_live_set_plugins ON plugins.id = ableton_live_set_plugins.plugin_id
                WHERE ableton_live_set_plugins.ableton_live_set_id = ?
            """, (ableton_set_id,))
            plugins = cursor.fetchall()
            
            plugins_combo = QComboBox()
            for plugin in plugins:
                plugins_combo.addItem(plugin[0])
            self.results_table.setCellWidget(row_idx, self.friendly_headers.index('Plugins'), plugins_combo)

            cursor.execute("""
                SELECT name FROM samples
                JOIN ableton_live_set_samples ON samples.id = ableton_live_set_samples.sample_id
                WHERE ableton_live_set_samples.ableton_live_set_id = ?
            """, (ableton_set_id,))
            samples = cursor.fetchall()

            samples_combo = QComboBox()
            for sample in samples:
                samples_combo.addItem(sample[0])
            self.results_table.setCellWidget(row_idx, self.friendly_headers.index('Samples'), samples_combo)

        self.results_table.setColumnCount(len(self.headers))
        self.results_table.setHorizontalHeaderLabels(self.friendly_headers)
        
if __name__ == '__main__':
    app = QApplication(sys.argv)
    main_window = MainWindow()
    main_window.show()
    sys.exit(app.exec_())
