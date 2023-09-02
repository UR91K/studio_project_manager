
"""
Watchdog Observer for Ableton Live Set (.als) files.

This module sets up a file observer that monitors changes to Ableton Live Set (.als) files within a specified directory.
When a .als file is created, modified, or deleted, appropriate actions are triggered.

Modules/Classes:
    - FileSystemHandler (FileSystemEventHandler): A handler class that defines actions for when 
      a file is modified, created, or deleted.

Functions:
    - main(): Sets up and starts the file observer to monitor changes to .als files. It reads the directory
      to be monitored from a config.toml file and sets up a continuous loop to keep the observer active.

Imports:
    - pathlib: For handling and manipulating filesystem paths.
    - time: To induce sleep in the continuous loop in the `main` function.
    - os: For accessing system functionality like changing the working directory.
    - toml: For loading configuration settings from a toml file.
    - watchdog.observers and watchdog.events: For setting up the file observer and handling file events.
    - db_manager: Contains the database-related functions and the AbletonLiveSet model.
    - sqlalchemy.orm: For database session management.
    - logging_utility: Contains logging-related utilities.

Usage:
    Run this script to start the file observer. Use CTRL+C to stop the observer gracefully.
"""

import os
import pathlib
import time

import toml
from sqlalchemy.orm import Session
from watchdog.events import FileSystemEventHandler
from watchdog.observers import Observer

from db_manager import AbletonLiveSet, init_database, process_file
from logging_utility import log


class FileSystemHandler(FileSystemEventHandler):
    """
    Custom event handler for filesystem events related to Ableton Live Set (.als) files.

    This handler reacts to created, modified, and deleted events of .als files.
    For each of these events, it triggers specific actions depending on the event type.

    Attributes:
    - session (Session): A SQLAlchemy session to interact with the database.

    Methods:
    - on_modified(event): Process .als files when they are modified.
    - on_created(event): Process .als files when they are created.
    - on_deleted(event): Remove the corresponding entry from the database when an .als file is deleted.
    """
    
    def __init__(self, session: Session):
        self.session = session

    def on_modified(self, event):
        if event.is_directory:
            return
        path = pathlib.Path(event.src_path)
        if path.suffix == ".als":
            process_file(path, self.session)

    def on_created(self, event):
        if event.is_directory:
            return
        path = pathlib.Path(event.src_path)
        if path.suffix == ".als":
            process_file(path, self.session)

    def on_deleted(self, event):
        if event.is_directory:
            return
        path = pathlib.Path(event.src_path)
        if path.suffix == ".als":
            existing_entry = self.session.query(AbletonLiveSet).filter_by(path=str(path)).first()
            if existing_entry:
                self.session.delete(existing_entry)
                self.session.commit()

def main():
    os.chdir(os.path.dirname(os.path.abspath(__file__)))
    config = toml.load("config.toml")
    user_home_dir = os.path.expanduser("~")
    DIR = pathlib.Path(config["directories"]["paths"][0].replace("{USER_HOME}", user_home_dir))
    DATABASE_PATH = pathlib.Path(config["database_path"]["path"].replace("{USER_HOME}", user_home_dir))
    session = init_database(DATABASE_PATH)

    observer = Observer()
    observer.schedule(FileSystemHandler(session), path=DIR, recursive=True)
    observer.start()
    log.info("File Observer started... CTRL+C to exit.")
    try:
        while True:
            time.sleep(1)
    except KeyboardInterrupt:
        observer.stop()
    observer.join()

if __name__ == "__main__":
    main()