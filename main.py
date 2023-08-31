import os
import subprocess
import toml

if __name__ == '__main__':

    os.chdir(os.path.dirname(os.path.abspath(__file__)))
    config = toml.load("config.toml")
    user_home_dir = os.path.expanduser("~")
    if os.path.exists(config["database_path"]["path"].replace("{USER_HOME}", user_home_dir)):
        os.remove(config["database_path"]["path"].replace("{USER_HOME}", user_home_dir))

    process = subprocess.Popen(['python', os.path.join('db_manager.py')])
    process.wait()
    process = subprocess.Popen(['python', os.path.join('file_watcher.py')])
    # commented out because it doesn't work properly right now.
    # subprocess.call(['python', os.path.join('gui.py')])