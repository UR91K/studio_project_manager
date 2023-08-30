from sqlalchemy import create_engine
from sqlalchemy.orm import declarative_base, sessionmaker
import toml
import os
import pathlib

os.chdir(os.path.dirname(os.path.abspath(__file__)))
config = toml.load("config.toml")

Base = declarative_base()

user_home_dir = os.path.expanduser("~")
DATABASE_PATH = pathlib.Path(config["database_path"]["path"].replace("{USER_HOME}", user_home_dir))
engine = create_engine(f'sqlite:///{DATABASE_PATH}', echo=True)

Session = sessionmaker(bind=engine)

def get_session():
    return Session()

def create_tables():
    Base.metadata.create_all(engine)