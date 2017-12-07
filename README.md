## GitterGtk/GtkGitter
### Written by [seanr707](https://github.com/seanr707)

![](screenshots/gitter_gtk-no_sidebar.png?raw=true)

**Image shown is with sidebar minimized**

Features:

* Receives messages to any subscribed Gitter.im repos and private chats
* Can send (single-line) messages from account
* Uses ~15MB memory to run
* Sidebar to easily view and change chats
* Uses gtk-rs for a native Linux GUI

What is not yet implemented:

* Ability to edit messages
* Sending multi-line messages
* Parsing markdown and/or HTML from Gitter to Gtk's Pango markup language
* Caching/displaying avatars and images

Supported OS's:

* Linux x64 (this is my setup)
* Linux x32 (not yet tested, but I presume the code should port easily)

Untested OS's (GTK can be setup, but I have not yet researched how to implement this yet):

* Windows 32/64
* MacOS

This is an initial release, and also my first Rust project, and only my second venture into GTK development.

If you see any bugs or have any feature requests, report them here on Github.

If you want to add features, feel free to fork and send a pull request. Please try to have additional code formatted similar to the current base.

Thank you for taking a look at my project!

Sean R. Copyright 2017
