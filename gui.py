from PyQt5.QtWidgets import QApplication, QMainWindow, QVBoxLayout, QWidget, QLineEdit, QTableWidget, QTableWidgetItem, QComboBox
import sqlite3
from pathlib import Path
import toml
import os

# THIS FILE DOESNT FUCKING WORK RIGHT NOW

os.chdir(os.path.dirname(os.path.abspath(__file__)))
config = toml.load("config.toml")
user_home_dir = os.path.expanduser("~")
DATABASE_PATH = Path(config["database_path"]["path"].replace("{USER_HOME}", user_home_dir))
conn = sqlite3.connect(DATABASE_PATH)
cursor = conn.cursor()
cursor.execute("PRAGMA table_info(ableton_live_sets)")
columns_info = cursor.fetchall()
columns_info_dict = {info[1]: index for index, info in enumerate(columns_info)}

cursor.execute("""
CREATE VIRTUAL TABLE IF NOT EXISTS ableton_live_sets_fts USING fts5(
    name, creator, key, major_minor_patch, tempo, 
    time_signature, estimated_duration, furthest_bar, 
    plugin_names, sample_names
);
""")

cursor.execute("""
INSERT INTO ableton_live_sets_fts 
SELECT name, creator, key, major_minor_patch, tempo, 
time_signature, estimated_duration, furthest_bar, 
plugin_names, sample_names FROM ableton_live_sets;
""")

conn.close()

excluded_columns = ["uuid", "identifier", "xml_root", "path", "file_hash", "last_scan_timestamp", "major_version", "minor_version", "creator"]

def perform_full_text_search(query):
    conn = sqlite3.connect(DATABASE_PATH)
    cursor = conn.cursor()
    
    # Use the MATCH query for full-text search
    cursor.execute("SELECT * FROM ableton_live_sets WHERE rowid IN (SELECT rowid FROM ableton_live_sets_fts WHERE ableton_live_sets_fts MATCH ?)", (query,))
    results = cursor.fetchall()
    
    conn.close()
    return results


class SearchApp(QMainWindow):
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
            "estimated_duration" : "Duration (estimated)",
            "furthest_bar" : "Furthest Bar",
            "plugin_names" : "Plugin Names",
            "sample_paths" : "Sample Paths"
        }

        self.db_columns = [info[1] for info in columns_info if info[1] not in excluded_columns]
        self.headers = [column_display_names.get(col, col) for col in self.db_columns]

        self.search_line_edit = QLineEdit(self)
        self.result_table = QTableWidget(self)
        self.result_table.setColumnCount(len(columns_info) - len(excluded_columns))
        # self.result_table.setHorizontalHeaderLabels(self.headers)

        layout = QVBoxLayout()
        layout.addWidget(self.search_line_edit)
        layout.addWidget(self.result_table)

        central_widget = QWidget(self)
        central_widget.setLayout(layout)
        self.setCentralWidget(central_widget)

        self.search_line_edit.textChanged.connect(self.perform_search)
        self.result_table.setHorizontalHeaderLabels(self.headers)

        print("self.headers: ", self.headers)
        if "Plugin Names" in self.headers:
            self.result_table.setColumnWidth(self.headers.index("Plugin Names"), 200)
        if "Sample Names" in self.headers:
            self.result_table.setColumnWidth(self.headers.index("Sample Paths"), 200)


    def perform_search(self):
        query = self.search_line_edit.text()
        
        if not query:
            self.result_table.setRowCount(0)
            return
        
        matches = perform_full_text_search(query)
        
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


if __name__ == "__main__":
    app = QApplication([])
    window = SearchApp()
    window.show()
    app.exec_()