import pathlib
import time
import os
import toml
from watchdog.observers import Observer
from db_manager import process_file, AbletonLiveSet, init_database
from watchdog.events import FileSystemEventHandler
from sqlalchemy.orm import Session
from logging_utility import log


class FileSystemHandler(FileSystemEventHandler):

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
    DIR = config["directories"]["paths"][0]
    user_home_dir = os.path.expanduser("~")
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