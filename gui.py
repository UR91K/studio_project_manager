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

# currently not functional!

from PyQt5.QtWidgets import QApplication, QMainWindow, QVBoxLayout, QWidget, QLineEdit, QTableWidget, QTableWidgetItem, QComboBox
from PyQt5.QtCore import QTimer
import sqlite3
from pathlib import Path
import toml
import os

os.chdir(os.path.dirname(os.path.abspath(__file__)))
config = toml.load("config.toml")
user_home_dir = os.path.expanduser("~")
DATABASE_PATH = Path(config["database_path"]["path"].replace("{USER_HOME}", user_home_dir))
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
                    "creator"
                    ]


def setup_fts_for_ableton_live_sets():
    conn = sqlite3.connect(DATABASE_PATH)
    cursor = conn.cursor()
    
    # Create the FTS virtual table for ableton_live_sets if it doesn't exist
    cursor.execute("""
    CREATE VIRTUAL TABLE IF NOT EXISTS ableton_live_sets_fts USING fts5(
        identifier UNINDEXED,
        name, 
        creator, 
        key, 
        major_minor_patch, 
        tempo, 
        time_signature, 
        estimated_duration, 
        furthest_bar
    );
    """)
    
    # Populate the FTS table from the main ableton_live_sets table
    cursor.execute("""
    INSERT OR IGNORE INTO ableton_live_sets_fts 
    SELECT identifier, name, creator, key, major_minor_patch, tempo, 
           time_signature, estimated_duration, furthest_bar 
    FROM ableton_live_sets;
    """)
    
    conn.commit()
    conn.close()


def hybrid_search(search_term):
    conn = sqlite3.connect(DATABASE_PATH)
    cursor = conn.cursor()
    
    # Step 1: Use FTS to get relevant ableton_live_sets identifiers
    cursor.execute("""
    SELECT identifier FROM ableton_live_sets_fts 
    WHERE name MATCH ?
    """, (search_term,))

    
    matching_identifiers = [row[0] for row in cursor.fetchall()]
    
    # If no matches, return an empty list
    if not matching_identifiers:
        return []
    
    # Step 2: Use the matching identifiers to join with plugins and samples
    placeholders = ",".join("?" * len(matching_identifiers))
    cursor.execute(f"""
    SELECT 
        als.path, als.name, als.creation_time, als.last_modification_time,
        als.key, als.major_minor_patch, als.tempo, als.time_signature,
        als.estimated_duration, als.furthest_bar,
        group_concat(DISTINCT p.name, ', ') as plugin_names,
        group_concat(DISTINCT s.name, ', ') as sample_names
    FROM ableton_live_sets als
    LEFT JOIN ableton_live_set_plugins alsp on als.identifier = alsp.ableton_live_set_id
    LEFT JOIN plugins p on alsp.plugin_id = p.id
    LEFT JOIN ableton_live_set_samples alss on als.identifier = alss.ableton_live_set_id
    LEFT JOIN samples s on alss.sample_id = s.id
    WHERE als.identifier IN ({placeholders})
    GROUP BY als.identifier
    """, matching_identifiers)
    
    results = cursor.fetchall()
    
    conn.close()
    return results


class MainWindow(QMainWindow):
    def __init__(self):
        super().__init__()

        self.setWindowTitle("Studio Project Manager")
        
        column_display_names = {
            "name": "Project Name",
            "creation_time": "Creation Date",
            "last_modification_time": "Last Modified",
            "creator" : "Creator",
            "key" : "Key",
            "major_minor_patch" : "Ableton Version",
            "tempo" : "Tempo",
            "time_signature" : "Time Signature",
            "estimated_duration" : "Duration",
            "furthest_bar" : "Furthest Bar",
            "plugin_names" : "Plugin Names",
            "sample_paths" : "Sample Paths"
        }

        self.db_columns = [info[1] for info in columns_info if info[1] not in excluded_columns]
        self.headers = [column_display_names.get(col, col) for col in self.db_columns]

        self.search_line_edit = QLineEdit(self)
        self.search_line_edit.setPlaceholderText("Enter search term...")
        
        # Set up the debounce timer
        self.debounce_timer = QTimer(self)
        self.debounce_timer.setSingleShot(True)  # Ensure the timer only runs once after starting
        self.debounce_timer.timeout.connect(self.perform_search)  # Connect the timer's timeout signal to perform_search
        
        # Connect the textChanged signal to the debounce function
        self.search_line_edit.textChanged.connect(self.debounce)

        self.result_table = QTableWidget(self)
        self.result_table.setColumnCount(len(columns_info) - len(excluded_columns))
        self.result_table.setHorizontalHeaderLabels(self.headers)

        layout = QVBoxLayout()
        layout.addWidget(self.search_line_edit)
        layout.addWidget(self.result_table)

        central_widget = QWidget(self)
        central_widget.setLayout(layout)
        self.setCentralWidget(central_widget)

        # Adjust column widths
        if "Plugin Names" in self.headers:
            self.result_table.setColumnWidth(self.headers.index("Plugin Names"), 200)
        if "Sample Names" in self.headers:
            self.result_table.setColumnWidth(self.headers.index("Sample Paths"), 200)

    def debounce(self):
        # Stop the timer if it's running
        self.debounce_timer.stop()
        
        # Start or restart the timer
        self.debounce_timer.start(300)  # 300ms debounce delay

    def perform_search(self):
        query = self.search_line_edit.text()
        
        if not query:
            self.result_table.setRowCount(0)
            return
        
        matches = hybrid_search(query)
        
        self.result_table.setRowCount(len(matches))
        for row, match_row in enumerate(matches):
            for col_index, value in enumerate(match_row):
                column_name = columns_info[col_index][1]
                if column_name in self.db_columns:
                    display_index = self.db_columns.index(column_name)
                    
                    # Check if the column is for plugins or samples
                    if column_name == "plugin_names":
                        combo = QComboBox()
                        for plugin in value.split(", "):
                            combo.addItem(plugin)
                        self.result_table.setCellWidget(row, display_index, combo)
                    elif column_name == "sample_paths":
                        combo = QComboBox()
                        for sample in value.split(", "):
                            combo.addItem(sample)
                        self.result_table.setCellWidget(row, display_index, combo)
                    else:
                        self.result_table.setItem(row, display_index, QTableWidgetItem(str(value)))


    def display_results(self, results):
        # Modified to display plugin_names and sample_paths as tooltips
        for row, result in enumerate(results):
            for col, item in enumerate(result):
                new_item = QTableWidgetItem(str(item))
                if self.headers[col] == "Plugin Names" or self.headers[col] == "Sample Paths":
                    new_item.setToolTip(item)
                self.result_table.setItem(row, col, new_item)
        # Adjust column widths here
        if "Plugin Names" in self.headers:
            self.result_table.setColumnWidth(self.headers.index("Plugin Names"), 200)
        if "Sample Names" in self.headers:
            self.result_table.setColumnWidth(self.headers.index("Sample Paths"), 200)

if __name__ == "__main__":
    setup_fts_for_ableton_live_sets()
    app = QApplication([])
    window = MainWindow()
    window.show()
    app.exec_()