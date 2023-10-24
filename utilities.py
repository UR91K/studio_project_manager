
"""
This file contains helper functions, type decorators for the SQLAlchemy models, and exceptions
"""

import datetime
import pathlib
import uuid
from typing import List
from xml.etree import ElementTree

import toml
from sqlalchemy import String, Text, TypeDecorator

import utilities
from logging_utility import log


def add_directory(new_directory: str):
    """
    Add a new directory path to the 'directories' section in the 'config.toml' file.
    
    This function loads the existing 'config.toml' file, checks if the 'directories' section exists,
    and then checks if the provided new_directory is already listed. If not, it appends the new_directory
    to the 'paths' list under the 'directories' section. Changes are then written back to the 'config.toml' file.
    
    Args:
    - new_directory (str): The directory path to be added to the 'config.toml' file.
    
    Returns:
    None
    
    Raises:
    - FileNotFoundError: If the 'config.toml' file doesn't exist.
    - toml.TomlDecodeError: If there's an error decoding the 'config.toml' file.
    """
    config = toml.load("config.toml")
    if "directories" not in config:
        config["directories"] = {"paths": []}

    if new_directory not in config["directories"]["paths"]:
        config["directories"]["paths"].append(new_directory)
    
    with open("config.toml", "w") as toml_file:
        toml.dump(config, toml_file)


def remove_directory(directory_to_remove: str):
    """
    Remove a directory path from the 'directories' section in the 'config.toml' file.
    
    The function reads the 'config.toml' file, verifies if the directory_to_remove exists 
    in the 'directories' section, and if so, removes it. The updated configuration is then 
    written back to the 'config.toml' file.

    Args:
    - directory_to_remove (str): The directory path to be removed from the 'config.toml' file.
    
    Returns:
    None
    
    Raises:
    - FileNotFoundError: If the 'config.toml' file doesn't exist.
    - toml.TomlDecodeError: If there's an error decoding the 'config.toml' file.
    """

    config = toml.load("config.toml")
    
    if "directories" in config and directory_to_remove in config["directories"]["paths"]:
        config["directories"]["paths"].remove(directory_to_remove)

        with open("config.toml", "w") as toml_file:
            toml.dump(config, toml_file)


def load_directories_from_config() -> list:
    """
    Load directory paths from the 'directories' section of the 'config.toml' file.

    This function attempts to load the 'config.toml' file and extract the list of directory paths
    stored under the 'directories' section. If the 'config.toml' file does not exist, it creates a new one 
    with an empty 'directories' section.

    Returns:
    - list: A list of directory paths loaded from the 'config.toml' file. 
            Returns an empty list if no paths are found or if the config file is newly created.

    Raises:
    - toml.TomlDecodeError: If there's an error decoding the 'config.toml' file.
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
    """
    Custom SQLAlchemy type decorator for storing XML ElementTrees as strings in the database.

    This type decorator facilitates the storage of XML ElementTrees in the database as strings.
    When saving an XML ElementTree to the database, it's converted to a string, and when retrieving 
    it from the database, it's parsed back into an XML ElementTree.
    """
    
    impl = Text

    def process_bind_param(self, value, dialect):
        """
        Convert the XML ElementTree to a string before saving to the database.

        Args:
        - value: The XML ElementTree object to be stored.
        - dialect: The database dialect in use.

        Returns:
        - str or None: The XML string representation or None if the value is None.
        """
        if value is not None:
            return ElementTree.tostring(value, encoding='unicode')
        return None

    def process_result_value(self, value, dialect):
        """
        Convert the stored string back to an XML ElementTree after retrieving from the database.

        Args:
        - value: The stored XML string representation.
        - dialect: The database dialect in use.

        Returns:
        - ElementTree or None: The XML ElementTree object or None if the stored value is None.
        """
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
    """
    Custom SQLAlchemy type decorator for storing `pathlib.Path` objects as strings in the database.

    This type decorator facilitates the storage of `pathlib.Path` objects in the database as strings.
    When saving a `pathlib.Path` to the database, it's converted to its string representation, 
    and when retrieving it from the database, it's converted back into a `pathlib.Path` object.
    """
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