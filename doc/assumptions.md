# Assumptions

## Disambiguation pages

Disambiguation pages do not indicate an association consistent with the goals of six_degrees, so we need to handle disambiguation pages in a special manner.

1. Assume that a link including the word "disambiguation" in parenthesis is a pointer to a disambiguation page
2. If the title of a page includes the word disambiguation" in parenthesis, assume that this is a disambiguation page
3. If 75% of the links on a page contain a phrase that matches the title of the page, assume that this is a disambiguation page

### _Handling disambiguation_

1. Do not record links that point to a disambiguation page
2. Do not follow links that point to a disambiguation page
3. If a loaded page is identified as a disambiguation page (probably because of #3 above) discard all links out of the page before pushing to a slab and saving to cache

## Redirects

1. If a page includes a link in namespace 4 with the phrase "categorizing_redirects", this is a redirect (see <https://en.wikipedia.org/wiki/Wikipedia:Redirect>).

### _Handling redirects_

1. Load the redirect page
2. Save the reidrect page to cache, to prevent reloading from Wikimedia if other pages link into the redirect page
3. Identify the appropriate forward link in the redirect page - this should be the only link in namespace 0.
3. Discard the redirect page, and use the forward link as the referenced page in the slab.

## Updating the page cache

The date the page was most recently loaded from Wikipedia is saved in the Entry struct. If the page is more than 3 months old, then there is a 1% chance that it wil be reloaded rather than read from cache

### _Cache implementation_

Pick random number from 0..99 and reload the page if the number is zero. This will result in a gradual maintenance of the pages, with the more commonly referenced pages more likely to be refreshed.

##  Weak-link pages:
Some pages (e.g. _wife_) will provide weak-links between two other pages, providing links between those pages that would not normally be linked. Unlike disambiguation pages Wikipedia provides no mechanisms to detect fake-hubs. We will assume that a page with an inbound link count that is more than 200% of the outbound links from that page is a weak-link page, and will eliminate links that occur only through that page (links into and links out of the page still count; just links through are eliminated). A run-time option can be used to adjust the threshold for weak-link page detection. 

## Loading a page

For each link on the page

1. Visit the referenced page.

1. If it exists in the appropriate slab, add a back-link from the referenced page to the referencing page

  2 If the referenced page does not exist, create a stub entry in the appropriate slab, add a back-link from the referenced page to the referencing page
  
2. If the referenced page is not at the search depth, recurse for every link in the referenced page

#Assumptions
1. Disambiguation pages: If 75% of the links on a page contain a word that matches the title of the page, assume that this is a disambiguation page and discard all links into and out of the page. Disambiguation pages do not indicate an association consistent with the goals of six_degrees. If a disambiguation page is loaded, save the page with
  1. an identifier that marks it as a disambiguation page, so that it's not constantly reloaded from Wikipedia
  2. no inbound or outbound links
2.
3. The date the page was most recently loaded from Wikipedia is saved in the Entry struct. If the page is more than 3 months old, then there is a 1% chance that it wil be reloaded rather than read from cache (pick random number from 0..99 and reload the page if the number is zero). This will result in a gradual maintenance of the pages, with the more common pages more likely to be refreshed.