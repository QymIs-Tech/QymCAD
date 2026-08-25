# Documents, templates and keeping your work

## Document properties

![Document properties: they travel with the file.](img/doc-props.png)

Title, author, version, comment and the **geometry accuracy** travel with the file: whoever opens it
sees whose it is and at what accuracy to compute. The creation date is stamped once, at the first
save.

## Templates

A template is an ordinary document saved as a starting point: properties, accuracy, datums, prepared
sketches. “File -> New from a template” creates a new document from it.

The new document **does not remember the template's path**: the first Save asks where to put it, and
the template is unharmed.

Templates are files in the settings folder — you can share them.

## Autosave

Every so often the program silently writes a copy next to the project. A normal Save removes it. If
the copy turns out to be newer than the file, the previous session was cut short after some edits,
and the program offers to restore.

The period is set in the settings; zero turns it off.

## Recent files

The “File -> Recent” menu and the start screen. Paths that vanished from the disk are dropped from the
list so that it does not offer what is not there.

## Undo

Ctrl+Z and Ctrl+Y, with the name of the step in the Edit menu. The depth is set in the settings: more
steps means more memory, a snapshot of a large assembly weighs tens of megabytes.
