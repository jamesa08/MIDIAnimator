import logging
from io import StringIO

# Create and configure the logger
logger = logging.getLogger('MIDIAnimator')
logger.setLevel(logging.INFO)
buffer = StringIO()

# clear the handlers from the logger before adding new ones
logger.handlers.clear()

# Create a custom log formatter
class ColoredFormatter(logging.Formatter):
    COLORS = {
        'DEBUG': '\033[36m',    # Cyan
        'INFO': '\033[32m',     # Green
        'WARNING': '\033[33m',  # Yellow
        'ERROR': '\033[31m',    # Red
        'CRITICAL': '\033[1;31m'  # Bold Red
    }
    RESET = '\033[0m'

    def format(self, record):
        levelname = record.levelname
        if levelname in self.COLORS:
            record.levelname = f'{self.COLORS[levelname]}{levelname}{self.RESET}'
        return super().format(record)

# add two stream handlers, one to the buffer and one to the console
bufferStream = logging.StreamHandler(buffer)
bufferStream.setFormatter(logging.Formatter('%(name)s - %(levelname)s - %(message)s'))
logger.addHandler(bufferStream)

consoleStream = logging.StreamHandler()
consoleStream.setFormatter(ColoredFormatter('%(name)s - %(levelname)s - %(message)s'))
logger.addHandler(consoleStream)
