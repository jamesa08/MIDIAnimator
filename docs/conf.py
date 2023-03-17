# Configuration file for the Sphinx documentation builder.

import os
import sys
import re
import datetime

print(f"cwd={os.getcwd()}")
sys.path.insert(0, os.path.abspath("../"))

from MIDIAnimator import bl_info

# -- Project information

project = 'MIDIAnimator'
copyright = f'{datetime.date.today().year}, James Alt'
author = 'James Alt'

release = bl_info['name'].split(" ")[-1]
version = bl_info['name'].split(" ")[-1]

# -- General configuration

extensions = [
    'sphinx.ext.duration',
    'sphinx.ext.doctest',
    'sphinx.ext.autodoc',
    'sphinx.ext.autosummary',
    'sphinx.ext.intersphinx',
    'myst_parser'
]

intersphinx_mapping = {
    'python': ('https://docs.python.org/3/', None),
    'sphinx': ('https://www.sphinx-doc.org/en/master/', None),
}
intersphinx_disabled_domains = ['std']

templates_path = ['_templates']

# -- Options for HTML output

html_theme = 'sphinx_rtd_theme'

# -- Options for EPUB output
epub_show_urls = 'footnote'

autodoc_mock_imports = ["MIDIAnimator.libs", "numpy"]

autodoc_class_signature = "separated"

add_module_names = False

exclude_patterns = ['api/modules.rst', 'api/MIDIAnimator.rst', 'api/MIDIAnimator.ui.rst']

master_doc = 'index'


# thank you to https://github.com/sphinx-doc/sphinx/issues/4065#issuecomment-538535280
def strip_signatures(app, what, name, obj, options, signature, return_annotation):
    sig = None                                                                  
    if signature is not None:                                                   
        sig = re.sub('MIDIAnimator\.[^.]*\.', '', signature)                           
                                                                                
    ret = None                                                                  
    if return_annotation is not None:                                           
        ret = re.sub('MIDIAnimator\.[^.]*\.', '', signature)                           
                                                                                
    return sig, ret                                                             
                                                                                
                                                                                
def setup(app):
    app.connect('autodoc-process-signature', strip_signatures)