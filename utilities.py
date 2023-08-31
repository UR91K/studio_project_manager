import uuid
import pathlib
import datetime
from xml.etree import ElementTree
from sqlalchemy import String, TypeDecorator, Text
import toml
from logging_utility import log
import utilities
from typing import List


def add_directory(new_directory: str):
    config = toml.load("config.toml")
    if "directories" not in config:
        config["directories"] = {"paths": []}

    if new_directory not in config["directories"]["paths"]:
        config["directories"]["paths"].append(new_directory)
    
    with open("config.toml", "w") as toml_file:
        toml.dump(config, toml_file)


def remove_directory(directory_to_remove: str):
    config = toml.load("config.toml")
    
    if "directories" in config and directory_to_remove in config["directories"]["paths"]:
        config["directories"]["paths"].remove(directory_to_remove)

        with open("config.toml", "w") as toml_file:
            toml.dump(config, toml_file)


def load_directories_from_config() -> list:
    """
    Load directories from the config TOML file.
    """
    try:
        config = toml.load("config.toml")
        return config.get("directories", {}).get("paths", [])
    except FileNotFoundError:
        log.info("No config file found; creating new config file.")
        with open("config.toml", "w") as config_file:
            toml.dump({"directories": {"paths": []}}, config_file)
        return []


def get_als_files_from_dir(directory: pathlib.Path, search_subfolders: bool) -> List[pathlib.Path]:
    """Returns .als files from a directory."""
    if not directory.is_dir():
        raise InvalidPathError(f"'{directory}' is not a directory.")
    
    files = (
        list(directory.rglob("*.als"))
        if search_subfolders
        else list(directory.glob("*.als"))
    )
    return files


def filter_als_files(files: List[pathlib.Path]) -> List[pathlib.Path]:
    """Filter out unwanted .als files."""
    return [
        file
        for file in files
        if all(x not in file.parts[:-1] for x in ["Backup", "backup"])
        and not file.stem.startswith(("._"))
    ]


def get_als_paths(path: str, search_subfolders: bool = False) -> List[pathlib.Path]:
    """Returns a list of pathlib.Path objects for Ableton Live Set files (.als) found in a given path.

    Args:
        path (str): Full path to an Ableton Live Set file (.als) or directory containing them.
        search_subfolders (bool): Whether or not to search subfolders of the directory. Defaults to False.

    Returns:
        List[pathlib.Path]: List of pathlib.Path objects for Ableton Live Set files (.als) found.
    """
    path_obj = pathlib.Path(path)

    if path_obj.is_file():
        if path_obj.suffix != ".als":
            raise utilities.InvalidPathError(f"'{path}' is not a .als file.")
        return [path_obj]

    files = get_als_files_from_dir(path_obj, search_subfolders)
    return filter_als_files(files)


#TYPE DECORATORS
class TimeSignatureType(TypeDecorator):
    """
    Stores a tuple as a comma-separated string.
    """
    impl = String

    def process_bind_param(self, value, dialect):
        if value is not None:
            return '/'.join(map(str, value))

    def process_result_value(self, value, dialect):
        if value is not None:
            return tuple(value.split('/'))

    def copy(self, **kw):
        return TimeSignatureType(self.impl.length)


class VersionType(TypeDecorator):
    """
    Stores a tuple as a comma-separated string.
    """
    impl = String

    def process_bind_param(self, value, dialect):
        if value is not None:
            return '.'.join(map(str, value))

    def process_result_value(self, value, dialect):
        if value is not None:
            return tuple(value.split('.'))

    def copy(self, **kw):
        return VersionType(self.impl.length)


class XMLElementType(TypeDecorator):
    impl = Text

    def process_bind_param(self, value, dialect):
        """Convert the XML ElementTree to a string before saving."""
        if value is not None:
            return ElementTree.tostring(value, encoding='unicode')
        return None

    def process_result_value(self, value, dialect):
        """Convert the string back to an XML ElementTree after loading."""
        if value is not None:
            return ElementTree.fromstring(value)
        return None


class UUIDType(TypeDecorator):
    impl = String(36)  # UUIDs are 36 characters long when represented as strings

    def process_bind_param(self, value, dialect):
        """Convert the UUID to a string before saving."""
        if value is not None:
            if isinstance(value, uuid.UUID):
                return str(value)
            raise ValueError("Value is not a UUID instance.")
        return None

    def process_result_value(self, value, dialect):
        """Convert the string back to a UUID after loading."""
        if value is not None:
            return uuid.UUID(value)
        return None


class CommaSeparatedListType(TypeDecorator):
    """Store Python lists as comma-separated strings in the DB."""

    impl = String

    def process_bind_param(self, value, dialect):
        if value:
            return ", ".join(str(item) for item in value)
        return None

    def process_result_value(self, value, dialect):
        if value:
            return [item.strip() for item in value.split(",") if item.strip()]
        return []


class PathType(TypeDecorator):
    impl = String

    def process_bind_param(self, value, dialect):
        if value is not None:
            return str(value)
        return None

    def process_result_value(self, value, dialect):
        if value is not None:
            return pathlib.Path(value)
        return None


class TimeDeltaType(TypeDecorator):
    """Converts a timedelta to a string formatted as HHH:MM:SS.SSSSSS and vice versa."""
    
    impl = String

    def process_bind_param(self, value, dialect):
        if value is not None:
            total_seconds = value.total_seconds()
            hours, remainder = divmod(total_seconds, 3600)
            minutes, seconds = divmod(remainder, 60)
            return "{:03}:{:02}:{:06.3f}".format(int(hours), int(minutes), seconds)
        return None

    def process_result_value(self, value, dialect):
        if value is not None:
            hours, minutes, seconds = map(float, value.split(':'))
            return datetime.timedelta(hours=hours, minutes=minutes, seconds=seconds)
        return None

# EXCEPTIONS
class ElementNotFound(Exception):
    """Element doesnt exist within the xml hierarchy where expected."""


class InvalidPathError(Exception):
    pass