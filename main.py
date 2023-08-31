import os
import signal
import subprocess
import sys

import toml

if __name__ == '__main__':

    os.chdir(os.path.dirname(os.path.abspath(__file__)))
    config = toml.load("config.toml")
    user_home_dir = os.path.expanduser("~")
    if os.path.exists(config["database_path"]["path"].replace("{USER_HOME}", user_home_dir)):
        os.remove(config["database_path"]["path"].replace("{USER_HOME}", user_home_dir))

    db_process = subprocess.Popen([sys.executable, os.path.join('db_manager.py')])
    db_process.wait()

    subprocess.Popen([sys.executable, os.path.join('file_watcher.py')])

    # Blocks until GUI is closed, then program ends
    subprocess.call([sys.executable, os.path.join('gui.py')])
