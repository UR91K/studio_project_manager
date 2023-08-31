# studio_project_manager
project scanner and indexer with a search engine gui
=======

# studio project manager

This application scans specified directories for Ableton ALS files, processes their contents, and adds relevant details to a SQLite database.

## Usage

1. Add your projects folder to the config.toml file, along with where you would like to store the database, example:

```toml
# if you change this file while the program is running, you need to restart the program for changes to take effect.

# I haven't added support for multiple project folders yet, so if you add multiple directories to the paths, only the first one will be scanned. 

[directories]
paths = [

    'C:\Users\user\Documents\Projects'

]

# use {USER_HOME} as a shortcut to your user folder

[database_path]
path = '{USER_HOME}\ableton_manager\ableton_live_sets.db'

[live_database_dir]
dir = '{USER_HOME}\AppData\Local\Ableton\Live Database'
```

2. run `pip install -r requirements.txt`

3. run `main.py`
>>>>>>> b867bd6 (gui doesnt work right now)
