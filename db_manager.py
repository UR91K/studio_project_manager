
"""
This file contains:
Data extraction related helper functions
Database model definitions - Plugin, Sample and AbletonLiveSet models
Methods for extracting data from .als files which can be found in the AbletonLiveSet model
Initial database population using initial_scan()

The database created by this code has 5 tables:
    ableton_live_sets - contains all the data extracted from the ableton live sets
    samples - a list of all the samples used in at least one of the scanned projects
    plugins - a list of all the plugins used in at least one of the scanned projects
    ableton_live_set_plugins
    ableton_live_set_samples
"""

import gzip
import os
import pathlib
import re
import subprocess
import datetime
import xml
import uuid
import hashlib
import utilities
import toml
from collections import Counter
from typing import Any, List, Optional, Tuple
from xml.etree import ElementTree
from xml.etree.ElementTree import Element
from functools import wraps
from logging_utility import log
from sqlalchemy import create_engine, Column, Integer, String, DateTime, Float, ForeignKey, Table, UniqueConstraint, Boolean, MetaData, Index
from sqlalchemy.orm import sessionmaker, Session, relationship, object_session
from database_config import get_session, create_tables, Base


def above_version(supported_version):
    """
    Decorator to handle function support for changing XML schemas across Ableton versions.

    Args:
        supported_version: A tuple of three integers representing the major, minor, and patch version numbers
            that the decorated function is supported from.
    Returns:
        A decorator that wraps another function.
    """

    def decorator(func):
        @wraps(func)
        def wrapper(*args, **kwargs):
            if args[0].major_minor_patch < supported_version:
                version_name = f"Ableton Live {supported_version[0]}.{supported_version[1]}.{supported_version[2]}"
                log.warning(
                    f"{func.__name__} is only supported for {version_name} and above."
                )
                return None
            return func(*args, **kwargs)

        return wrapper

    return decorator


def version_supported(live_set_version: str, supported_version: str) -> bool:
    """
    Determines whether the live_set_version is supported by comparing it with the
    supported_version string in the format "major.minor.bugfix".

    :param live_set_version: The version of the Live set to check, in the format "major.minor.bugfix".
    :type live_set_version: str

    :param supported_version: The version of Live that is supported, in the format "major.minor.bugfix".
    :type supported_version: str

    :return: True if the live_set_version is equal to or older than the supported_version, False otherwise.
    :rtype: bool
    """
    for set_v, supported_v in zip(
        live_set_version.split("."), supported_version.split(".")
    ):
        set_v, supported_v = int(set_v), int(supported_v)
        if set_v > supported_v:
            return True
        elif set_v < supported_v:
            return False
    return True


def init_database(path):
    database_url = f"sqlite:///{path}"
    engine = create_engine(database_url, echo=True)
    Base.metadata.create_all(engine)
    Session = sessionmaker(bind=engine)
    return Session()


def get_note_symbol(number) -> str:
    """
    Returns the musical symbol for a given MIDI note number.

    Args:
        number (int): The MIDI note number.

    Returns:
        str: The musical symbol for the given MIDI note number.
    """
    notes = [
        "C", 
        "C#/Db",
        "D",
        "D#/Eb",
        "E",
        "F",
        "F#/Gb",
        "G",
        "G#/Ab",
        "A",
        "A#/Bb",
        "B",
    ]
    return notes[number % 12]


def find_most_frequent(lst: List[Any]) -> Any:
    """
    Find the most frequently occurring item in a list.

    Args:
        lst: A list of elements of any type.

    Returns:
        The most frequent element in the list. If there are multiple elements
        that occur with the same frequency, return the first element in the
        original list.

    Raises:
        ValueError: If the input list is empty.
    """
    if not lst:
        raise ValueError("Input list is empty!")

    counter = Counter(lst)
    most_common = counter.most_common()
    
    if most_common[0][1] == most_common[-1][1]:
        return lst[0]
    else:
        return most_common[0][0]


def get_element(
    root: Element,
    attribute_path: str,
    attribute: Optional[str] = None,
    silent_error: bool = False,
) -> Optional[Element]:
    """
    Returns an XML element using ElementTree XPath syntax.

    Args:
        root: The root element of the XML tree to search for the specified
            element.
        attribute_path: The path to the element using XPath syntax. The path
            should be a string with elements separated by dots, e.g.
            "parent.child.grandchild".
        attribute: Optional. If specified, the function will return the value
            of the specified attribute of the element, rather than the element
            itself.
        silent_error: Optional. If True, the function will return None if the
            element is not found, rather than raising an exception.

    Returns:
        The element or attribute of the specified path, or None if the path
        doesn't exist and silent_error is True.

    Raises:
        ElementNotFound: If the element is not found and silent_error is
        False.
    """
    element = root.findall(f"./{'/'.join(attribute_path.split('.'))}")
    if not element:
        if silent_error:
            return None
        xml.etree.ElementTree.dump(root)
        raise utilities.ElementNotFound(f"No element for path [{attribute_path}]")
    if attribute:
        return element[0].get(attribute)
    return element[0]

staticmethod
def get_file_hash(file_path):
    sha256_hash = hashlib.sha256()
    
    with open(file_path,"rb") as f:
        for byte_block in iter(lambda: f.read(4096),b""):
            sha256_hash.update(byte_block)
    
    return sha256_hash.hexdigest()


def process_file(path: pathlib.Path, session: Session):
    current_hash = get_file_hash(str(path))

    existing_entry_by_path = session.query(AbletonLiveSet).filter_by(path=str(path)).first()
    existing_entry_by_hash = session.query(AbletonLiveSet).filter_by(file_hash=current_hash).first()

    if existing_entry_by_path:
        existing_entry_by_path.parse_all()
        existing_entry_by_path.file_hash = current_hash
        session.commit()
    elif existing_entry_by_hash:
        existing_entry_by_hash.path = str(path)
        session.commit()
    else:
        ableton_live_set = AbletonLiveSet(path)
        session.add(ableton_live_set)
        ableton_live_set.parse_all()
        ableton_live_set.file_hash = current_hash
        session.commit()


def standardized_string(string):
    string = string.split('.')[0]
    string = string.lower()
    string = string.replace("stereo", "").replace("mono", "")
    string = string.replace("-", " ")
    string = string.replace("_", " ")
    string = " ".join(string.split())
    return string


def initial_scan(folder_path, session, search_subfolders=False):  
    paths = utilities.get_als_paths(folder_path, search_subfolders=search_subfolders)
    if not paths:
        log.error("No files to process; get_als_paths returned empty list")
        return False
    for path in paths:
        process_file(path, session)

def most_recent_db_file(directory):
    dir_path = pathlib.Path(directory)
    sorted_files = sorted(dir_path.glob('*.db'), key=lambda p: p.stat().st_mtime, reverse=True)
    return sorted_files[0] if sorted_files else None

def get_installed_plugins_from_ableton():
    latest_db_file = max((f for f in live_database_dir.iterdir() if f.suffix == '.db'), key=lambda p: p.stat().st_mtime, default=None)
    print(f"latest_db_file: {latest_db_file}")
    ableton_db = AbletonDatabase(latest_db_file)
    return ableton_db.get_installed_plugins()

# other classes

script_dir = os.path.dirname(os.path.abspath(__file__))
config_path = os.path.join(script_dir, "config.toml")
config = toml.load(config_path)
user_home_dir = os.path.expanduser("~")
live_database_dir = pathlib.Path(config["live_database_dir"]["dir"].replace("{USER_HOME}", user_home_dir))

ableton_live_set_plugins = Table('ableton_live_set_plugins', Base.metadata,
    Column('ableton_live_set_id', Integer, ForeignKey('ableton_live_sets.identifier')),
    Column('plugin_id', Integer, ForeignKey('plugins.id')))

ableton_live_set_samples = Table('ableton_live_set_samples', Base.metadata,
    Column('ableton_live_set_id', Integer, ForeignKey('ableton_live_sets.identifier')),
    Column('sample_id', Integer, ForeignKey('samples.id')))

class AbletonDatabase:
    def __init__(self, db_path):
        self.engine = create_engine(f'sqlite:///{db_path}')
        self.metadata = MetaData()
        self.metadata.bind = self.engine

    def get_installed_plugins(self):
        plugins_table = Table('plugins', self.metadata, autoload_with=self.engine)
        
        with self.engine.connect() as connection:
            installed_plugins = connection.execute(plugins_table.select()).fetchall()
        
        return [plugin.name for plugin in installed_plugins]


class Plugin(Base):

    __tablename__ = 'plugins'

    id = Column(Integer, primary_key=True)
    name = Column(String, nullable=False)
    version = Column(String)  # VST or VST3
    installed = Column(Boolean, default=False)

    __table_args__ = (
        Index('idx_plugins_name', 'name'), 
        UniqueConstraint('name', 'version', name='_name_version_uc')
        )
    ableton_live_sets = relationship('AbletonLiveSet', secondary=ableton_live_set_plugins, back_populates='plugins')
    
    installed_plugins_from_ableton = get_installed_plugins_from_ableton()

    def update_installation_status(self):
        if self.name in self.installed_plugins_from_ableton:
            self.installed = True
        else:
            self.installed = False

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.update_installation_status()


class Sample(Base):
    __tablename__ = 'samples'

    id = Column(Integer, primary_key=True)
    path = Column(String, unique=True)
    name = Column(String)
    is_present = Column(Boolean, default=True)

    ableton_live_sets = relationship('AbletonLiveSet', secondary='ableton_live_set_samples', back_populates='samples')


class AbletonLiveSet(Base):
    """
    This class represents an Ableton Live set.
    It contains methods to extract information about the set such as:
        project name, Ableton version, creation and modification time,
        sample and plugin names, tempo, key, and time signature.
    It also contains methods to parse and load the XML data of the Ableton Live set file
    and calculate the duration and furthest bar of the set using clip lengths.
    Additionally, it provides a method to open the folder containing the set in an Explorer window.
    """

    __tablename__ = 'ableton_live_sets'

    # database information
    uuid = Column(utilities.UUIDType, unique=True) # assuming you are using UUID4
    identifier = Column(Integer, primary_key=True, autoincrement=True)
    xml_root = Column(utilities.XMLElementType) # assuming it is stored as string
    path = Column(utilities.PathType)
    file_hash = Column(String, unique=True)
    last_scan_timestamp = Column(DateTime)

    # metadata
    name = Column(String)
    creation_time = Column(DateTime)
    last_modification_time = Column(DateTime)
    
    # extracted data
    creator = Column(String)
    key = Column(String)
    major_version = Column(Integer)
    minor_version = Column(Integer)
    major_minor_patch = Column(utilities.VersionType)
    tempo = Column(Float)
    time_signature = Column(utilities.TimeSignatureType)
    estimated_duration = Column(utilities.TimeDeltaType)
    furthest_bar = Column(Integer)

    plugins = relationship('Plugin', secondary=ableton_live_set_plugins, back_populates='ableton_live_sets')
    samples = relationship('Sample', secondary=ableton_live_set_samples, back_populates='ableton_live_sets')

    __table_args__ = (
        Index('idx_ableton_live_sets_name', 'name'),
        Index('idx_ableton_live_sets_creator', 'identifier'),
    )

    def __init__(self, pathlib_object) -> None:
        self.path = pathlib_object
        self.uuid = uuid.uuid4()
        self.file_hash = None
        self.identifier = None
        self.name = None
        self.last_modification_time = None
        self.creation_time = None
        self.xml_root = None
        self.creator = None
        self.major_version = None
        self.minor_version = None
        self.major_minor_patch = None
        self.tempo = None
        self.key = None
        self.furthest_bar = None
        # self.sample_paths = None
        self.time_signature = None


    def parse_all(self):
        """Parses all the data from the ableton project file in the correct order."""
        self.update_name()
        self.update_file_times()
        self.load_xml_data()
        self.load_version()
        self.update_tempo()
        self.update_furthest_bar()
        self.update_samples()
        self.update_plugins()
        self.update_key()
        self.update_time_signature()
        self.calculate_duration()
        self.generate_file_hash()


    def generate_file_hash(self):
        self.file_hash = get_file_hash(self.path)


    def update_creation_time(self) -> None:
        """
        Gets the creation time from the file attributes and sets the creation_time attribute.

        Args:
            ignore (bool): Flag indicating whether to update the creation time if it already exists.
        """
        if isinstance(self.creation_time, datetime.datetime):
            log.info(f"{self.name} ({str(self.uuid)[:5]}): creation time already exists, exiting method early")
            return
        try:
            self.creation_time = datetime.datetime.fromtimestamp(
                os.path.getctime(pathlib.Path(self.path))
            )
        except OSError:
            if not isinstance(self.creation_time, datetime.datetime):
                self.creation_time = datetime.datetime.now()
            log.warning(f"{self.name} ({str(self.uuid)[:5]}): Failed to update creation time due to invalid path.")
        log.info(f"{self.name} ({str(self.uuid)[:5]}): creation time updated to {self.creation_time}.")


    def update_name(self, custom_name: Optional[str] = None) -> str:
        """
        Updates the name of the project to the given custom name, otherwise uses the file name.

        Args:
            custom_name (str, optional): The custom name to use for the project. Defaults to None.

        Returns:
            str: The updated name of the project.
        """
        previous_name = self.name
        self.name = custom_name if custom_name else pathlib.Path(self.path).stem
        log.info(f"{self.uuid}: Project name updated from {previous_name} to {self.name}")
        return self.name


    def update_last_modification_time(self) -> None:
        """
        Gets last modification time from the file attributes and updates the last_modification_time attribute.
        """
        previous_modification_time = self.last_modification_time
        try:
            self.last_modification_time = datetime.datetime.fromtimestamp(
                os.stat(pathlib.Path(self.path)).st_mtime
            )
        except OSError as e:
            if self.last_modification_time is None:
                self.last_modification_time = datetime.datetime.now()
            log.warning(f"{self.name} ({str(self.uuid)[:5]}): {e}: Failed to update last modification time due to invalid path.")
        log.info(
            f"{self.name} ({str(self.uuid)[:5]}): last modification time updated from {previous_modification_time} to {self.last_modification_time}."
        )


    def update_file_times(self) -> None:
        """Updates both creation time and modification time.

        Returns:
            None
        """
        self.update_creation_time()
        self.update_last_modification_time()
        return None


    def show_folder(self) -> None:
        """Opens the folder at self.path in an Explorer window."""
        Popen_arg = f'explorer /select, "{pathlib.Path(self.path)}"'
        subprocess.Popen(Popen_arg)


    def load_version(self) -> None:
        """
        Extracts the version information of the Ableton Live Set and sets the major, minor, and patch version numbers as
        well as the major version number and minor version number separately.

        Returns:
            None
        """
        self.creator = self.xml_root.get("Creator")
        parsed = re.findall(
            r"Ableton Live ([0-9]{1,2})\.([0-9]{1,3})[\.b]{0,1}([0-9]{1,3}){0,1}",
            self.creator,
        )[0]
        parsed = [int(x) if x.isdigit() else x for x in parsed if x != ""]
        if len(parsed) == 3:
            major, minor, patch = parsed
        elif len(parsed) == 2:
            major, minor = parsed
            patch = 0
        else:
            log.error(f"{self.name} ({str(self.uuid)[:5]}): Could not parse version from: {self.creator}")
            return None
        self.major_minor_patch = major, minor, patch
        self.major_version = major
        self.minor_version = minor
        log.info(f"Set version: {self.creator}")
        if "b" in self.creator.split()[-1]:
            log.warning(
                "Set is from a beta version, some commands might not work properly!"
            )


    @staticmethod
    def human_readable_date(timestamp: float) -> str:
        """
        Returns a string representing a human-readable date.

        Parameters:
            timestamp (float): A UNIX timestamp in seconds since epoch.

        Returns:
            str: A string representing the timestamp in the format 'MM/DD/YYYY HH:MM:SS'.
        """
        return datetime.datetime.fromtimestamp(timestamp).strftime("%m/%d/%Y %H:%M:%S")


    def load_xml_data(self) -> Optional[Element]:
        """
        Load XML data from the path provided in self.path and set it to the self.root attribute.

        Returns:
        The ElementTree root element if the file is valid, otherwise None.
        """
        if not pathlib.Path(self.path).exists():
            log.error(f"{self.name} ({str(self.uuid)[:5]}): {pathlib.Path(self.path)} does not exist")
            self.xml_root = None
            return self.xml_root
        if not pathlib.Path(self.path).is_file():
            log.error(f"{self.name} ({str(self.uuid)[:5]}): {pathlib.Path(self.path)} is not a file")
            self.xml_root = None
            return self.xml_root
        if pathlib.Path(self.path).suffix != ".als":
            log.error(
                f"{self.name} ({str(self.uuid)[:5]}): {pathlib.Path(self.path)} is not a valid Ableton Live Set file"
            )
            self.xml_root = None
            return self.xml_root
        if self.last_modification_time is None or self.creation_time is None:
            log.warning(f"{self.name} ({str(self.uuid)[:5]}): ")
            self.update_file_times()
        with gzip.open(pathlib.Path(pathlib.Path(self.path)), "rb") as fd:
            data = fd.read()
            try:
                root = ElementTree.fromstring(data)
            except ElementTree.ParseError as e:
                log.error(f"{self.name} ({str(self.uuid)[:5]}): {pathlib.Path(self.path)} is not a valid XML file: {e}")
                self.xml_root = None
                return self.xml_root
        self.xml_root = root
        return self.xml_root


    def update_furthest_bar(self) -> float:
        """
        Updates the furthest bar variable of the project by computing the maximum
        end time of all clips and dividing it by the number of beats per bar.

        Returns:
            float: The updated furthest bar variable.
        """
        if self.time_signature:
            beats_per_bar, _ = self.time_signature
        else:
            log.error(f"{self.name} ({str(self.uuid)[:5]}): time_signature is None, defaulting beats_per_bar to 4.")
            beats_per_bar = 4

        previous = self.furthest_bar
        current_end_times = [
            float(end_times.get("Value"))
            for end_times in self.xml_root.iter("CurrentEnd")
        ]
        
        self.furthest_bar = (
            max(current_end_times) / beats_per_bar if current_end_times else 0.0
        )
        
        log.info(
            f"{self.name} ({str(self.uuid)[:5]}): updated furthest bar from {previous} to {self.furthest_bar}"
        )
        return self.furthest_bar



    @above_version(supported_version=(8, 2, 0))
    def update_tempo(self) -> None:
        """
        Updates the tempo of the project by extracting the tempo value from the XML file.

        Raises:
            XMLParsingError: If an error occurs while parsing the XML file.
        """
        previous_tempo = self.tempo
        post_10_tempo = "LiveSet.MasterTrack.DeviceChain.Mixer.Tempo.Manual"
        pre_10_tempo = "LiveSet.MasterTrack.MasterChain.Mixer.Tempo.ArrangerAutomation.Events.FloatEvent"
        major, minor, patch = self.major_minor_patch

        if major >= 10 or major >= 9 and minor >= 7:
            tempo_elem = get_element(
                self.xml_root, post_10_tempo, attribute="Value", silent_error=True
            )
        else:
            tempo_elem = get_element(self.xml_root, pre_10_tempo, attribute="Value")
        new_tempo = round(float(tempo_elem), 6)

        if new_tempo == previous_tempo:
            return

        self.tempo = new_tempo
        log.info(f"{self.name} ({str(self.uuid)[:5]}): updated tempo from {previous_tempo} to {self.tempo}")


    def parse_hex_path(self, text):
        """Takes raw hex string from XML entry and parses."""
        if not text:
            return None
        abs_hash_path = text.replace('\t', '').replace('\n', '')
        byte_data = bytearray.fromhex(abs_hash_path)
        try:
            return byte_data.decode('utf-16').replace('\x00', '')
        except UnicodeDecodeError as e:
            log.error(f'failed to decode path: {e}')
            return None


    def update_samples(self):
        """
        Updates the sample paths for the project based on the XML data.

        For Ableton versions earlier than 11, the method uses a complex procedure to 
        decode the sample path. For versions 11 and onward, the method fetches the path 
        directly from the XML.
        """
        sample_paths = set()

        if self.major_minor_patch[0] < 11:
            log.info('attempting to update sample paths')
            for sample_ref in self.xml_root.iter("SampleRef"):
                data_element = sample_ref.find("FileRef/Data")
                if data_element is None:
                    log.warning(f"{self.name} ({str(self.uuid)[:5]}): No data found in data_elem")
                    continue
                
                abs_hash_path = data_element.text.replace('\t', '').replace('\n', '')
                
                try:
                    byte_data = bytearray.fromhex(abs_hash_path)
                    path_string = byte_data.decode('utf-16').replace('\x00', '')
                except (AttributeError, UnicodeDecodeError, ValueError) as e:
                    log.error(f"{self.name} ({str(self.uuid)[:5]}): Error processing path: {e}")
                    continue

                sample_paths.add(pathlib.Path(path_string))
        else:
            for sample_ref in self.xml_root.iter("SampleRef"):
                path_elem = sample_ref.find("./FileRef/Path")
                if path_elem is not None:
                    path_value = path_elem.attrib["Value"]
                    sample_paths.add(pathlib.Path(path_value))
                else:
                    log.warning(f"{self.name} ({str(self.uuid)[:5]}): No path element found for sample reference")

        if not sample_paths:
            log.warning(f"{self.name} ({str(self.uuid)[:5]}): No sample paths detected")
            self.sample_paths = None
        else:
            session = object_session(self)

            for path in sample_paths:
                sample_name = path.name
                sample = session.query(Sample).filter_by(path=str(path)).first()

                if not sample:
                    sample = Sample(path=str(path), name=sample_name, is_present=True)
                    session.add(sample)
                    session.flush()

                if sample not in self.samples:
                    self.samples.append(sample)

            session.commit()


    def update_plugins(self):
        """
        Extracts the names of VST plugins from the project's XML data and associates them with the AbletonLiveSet instance.
        """
        vst_plugin_names = set()
        vst3_plugin_names = set()

        for plugin_info in self.xml_root.iter("Vst3PluginInfo"):
            name = plugin_info.find("Name")
            if name is not None:
                vst3_plugin_names.add(name.attrib["Value"])
        if not vst3_plugin_names:
            log.warning(f"{self.name} ({str(self.uuid)[:5]}): No Vst3PluginInfo names found")

        for plugin_info in self.xml_root.iter("VstPluginInfo"):
            name = plugin_info.find("PlugName")
            if name is not None:
                vst_plugin_names.add(name.attrib["Value"])
        if not vst_plugin_names:
            log.warning(f"{self.name} ({str(self.uuid)[:5]}): No VstPluginInfo names found")

        session = object_session(self)

        for plugin_name in vst_plugin_names:
            plugin = session.query(Plugin).filter_by(name=plugin_name, version="VST").first()
            if not plugin:
                plugin = Plugin(name=plugin_name, version="VST")
                session.add(plugin)
                session.flush()

            if plugin not in self.plugins:
                self.plugins.append(plugin)

        for plugin_name in vst3_plugin_names:
            plugin = session.query(Plugin).filter_by(name=plugin_name, version="VST3").first()
            if not plugin:
                plugin = Plugin(name=plugin_name, version="VST3")
                session.add(plugin)
                session.flush()

            if plugin not in self.plugins:
                self.plugins.append(plugin)

        session.commit()


    def _decode_numerator(self, encoded_value: int) -> int:
        """Decode numerator from the encoded time signature value."""
        if encoded_value < 0:
            return 1
        elif encoded_value < 99:
            return encoded_value + 1
        else:
            return (encoded_value % 99) + 1


    def _decode_denominator(self, encoded_value: int) -> int:
        """Decode denominator from the encoded time signature value."""
        multiple = encoded_value // 99 + 1
        return 2 ** (multiple - 1)


    def update_time_signature(self) -> Tuple[int, int]:
        """
        Updates the time signature of the project based on the Live Set file.

        Returns:
            A tuple representing the time signature of the project.
            The first element is the numerator of the time signature.
            The second element is the denominator of the time signature.
        """
        # This specific time value may represent a standard or default event time in MIDI. 
        # The value is completely undocumented, and I am basically assuming it is the correct value based on observations.
        TIME_SIGNATURE_EVENT_TIME = "-63072000"
        enum_event = self.xml_root.find(f'.//EnumEvent[@Time="{TIME_SIGNATURE_EVENT_TIME}"]')

        if enum_event is None:
            raise ValueError("Could not find EnumEvent with the specified time signature event time.")

        encoded_time_signature = int(enum_event.attrib["Value"])
        numerator = self._decode_numerator(encoded_time_signature)
        denominator = self._decode_denominator(encoded_time_signature)
        
        self.time_signature = (numerator, denominator)
        return self.time_signature


    def update_key(self) -> str:
        """
        Estimates the key of the project based on the most frequently used key and scale
        across all MIDI clips in the project. For Ableton versions earlier than 11, the 
        key is always set to "Unknown".

        Returns:
            str: The updated key of the project.
        """
        if not self.major_version:
            log.error(f"{self.name} ({str(self.uuid)[:5]}): Major version is not defined.")
            self.key = "Unknown"
            return self.key

        previous_key = self.key

        if self.major_version < 11:
            self.key = "Unknown"
        else:
            scale_dict = {}
            for midi_clip in self.xml_root.iter("MidiClip"):
                is_in_key_elem = midi_clip.find("IsInKey")
                if is_in_key_elem is not None and is_in_key_elem.attrib["Value"] == "true":
                    scale_info = midi_clip.find("ScaleInformation")
                    
                    if scale_info is None:
                        log.warning(f"{self.name} ({str(self.uuid)[:5]}): Missing ScaleInformation for a MIDI clip.")
                        continue

                    root_note_elem = scale_info.find("RootNote")
                    scale_name_elem = scale_info.find("Name")
                    
                    if root_note_elem and scale_name_elem:
                        root_note = get_note_symbol(int(root_note_elem.attrib["Value"]))
                        scale_name = scale_name_elem.attrib["Value"]
                        scale_dict[root_note] = scale_name
                    else:
                        log.warning(f"{self.name} ({str(self.uuid)[:5]}): Missing RootNote or Name for a MIDI clip's ScaleInformation.")
            
            scale_list = [f"{key} {value}" for key, value in scale_dict.items()]
            if scale_list:
                self.key = find_most_frequent(scale_list)
            else:
                self.key = "Unknown"

        log.info(f"{self.name} ({str(self.uuid)[:5]}): Updated key from {previous_key} to {self.key}.")
        return self.key


    def calculate_duration(self) -> datetime.timedelta:
        """
        Calculates the estimated duration of the project based on the time signature, tempo, and length in bars.
        Returns a timedelta object representing the estimated duration.
        """
        if not self.time_signature or not self.tempo or not self.furthest_bar:
            log.error(f"{self.name} ({str(self.uuid)[:5]}): Essential attributes (time_signature, tempo, furthest_bar) are missing or invalid for duration calculation.")
            return datetime.timedelta(seconds=0)

        beats_per_bar, _ = self.time_signature

        duration = (self.furthest_bar * beats_per_bar * 60) / self.tempo

        self.estimated_duration = datetime.timedelta(seconds=duration)
        log.info(f"{self.name} ({str(self.uuid)[:5]}): Estimated duration is {self.estimated_duration}.")

        return self.estimated_duration


def main():
    os.chdir(os.path.dirname(os.path.abspath(__file__)))
    config = toml.load("config.toml")
    create_tables()
    session = get_session()
    user_home_dir = os.path.expanduser("~")
    DIR = pathlib.Path(config["directories"]["paths"][0].replace("{USER_HOME}", user_home_dir))
    initial_scan(DIR, session, search_subfolders=True)

if __name__ == "__main__":
    main()