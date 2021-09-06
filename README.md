# maps-download-check
Checks the downloaded Here maps for the Jaguar I-PACE for corrupt parts. After removing the corrupt parts, the downloader only has to download the missing pieces.

## Instructions for use:
1. Start the Here maps downloader and download the maps as usual. Note: **when it reaches 100%, DO NOT close the downloader**
2. Start this `maps-download-check` tool and point it to the directory with the downloaded `update.xml` (on your USB stick)
3. Follow the instructions from the tool
4. Only when the tool is finished close the downloader. If some corrupt parts had to be removed, start again at step 1
