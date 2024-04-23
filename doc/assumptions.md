#Assumptions
1. Disambiguation pages: If 75% of the links on a page contain a word that matches the title of the page, assume that this is a disambiguation page and discard all links into and out of the page. Disambiguation pages do not indicate an association consistent with the goals of six_degrees. If a disambiguation page is loaded, save the page with
  1. an identifier that marks it as a disambiguation page, so that it's not constantly reloaded from Wikipedia
  2. no inbound or outbound links
2. The date the page was most recently loaded from Wikipedia is saved in the Entry struct. If the page is more than 3 months old, then there is a 1% chance that it wil be reloaded rather than read from cache (pick random number from 0..99 and reload the page if the number is zero). This will result in a gradual maintenance of the pages, with the more common pages more likely to be refreshed.