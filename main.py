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

    file_process = None
    try:
        file_process = subprocess.Popen([sys.executable, os.path.join('file_watcher.py')])
        file_process.wait()
        subprocess.call([sys.executable, os.path.join('gui.py')])
    except KeyboardInterrupt:
        # Handle Ctrl+C gracefully
        file_process.send_signal(signal.SIGINT)
