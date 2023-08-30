import logging
import pathlib
import datetime

VERSION = "1.0a"

log = logging.getLogger(__name__)
log.setLevel(logging.INFO)

ch = logging.StreamHandler()
ch.setLevel(logging.DEBUG)
formatter = logging.Formatter("%(asctime)s - %(name)s - %(levelname)s - %(message)s")
ch.setFormatter(formatter)
log.addHandler(ch)

log_dir = pathlib.Path.cwd() / "logs"
log_dir.mkdir(exist_ok=True)
log_name = f"LiveSetManager_v{VERSION}_{datetime.datetime.now().strftime('%Y-%m-%d_%H-%M-%S-%f')}.log"
fh = logging.FileHandler(log_dir / log_name)
fh.setLevel(logging.DEBUG)
fh.setFormatter(formatter)
log.addHandler(fh)
