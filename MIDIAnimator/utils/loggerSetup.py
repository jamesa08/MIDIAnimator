import logging

logger = logging.getLogger('MIDIAnimator')

stream = logging.StreamHandler()

stream.setLevel(logging.INFO)
formatter = logging.Formatter("%(name)s - %(levelname)s - %(message)s")

stream.setFormatter(formatter)

logger.addHandler(stream)