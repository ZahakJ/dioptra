# User calls

Ten calls of 30 minutes in weeks 5–6. The goal is facts about what people did, not
opinions about Dioptra. Ask about the last time, not "would you". Do not pitch; do not
demo until minute 25, and only if asked.

## Script (30 min)

**0–2 Open.**
"Thanks for the time. I'm trying to learn how people get from instrument data to a
figure. There are no right answers, and nothing to buy. Mind if I take notes?"

**2–10 The last figure.**

- Tell me about the last figure you made for a paper, poster or lab meeting. When was it?
- Where did the numbers come from? Which instrument, which file?
- Walk me through each step from that file to the figure. Which programs?
- Where did it take longest? What did you redo?
- Did anything go wrong (wrong values, lost formatting, resizing)? What did you do about it?
- Did the figure go through Illustrator, Inkscape or PowerPoint after? Why?

**10–16 The last analysis.**

- What was the last statistical test you ran? Who picked it?
- How did you check it was the right test?
- Has a reviewer or PI ever asked you to change an analysis or a figure? What happened?
- Do you ever run the same analysis on many files or plates? How often, and how?
- Plate readers: which one, which software, what format do you export? Who sets up the
  plate layout?

**16–22 Prism and money.**

- Which version of Prism do you use, and who pays for it?
- What happened at the last licence renewal? Who handled it? Did anyone lose access?
- What did you do the last time you couldn't get Prism (home, new lab, expired licence)?
- Have you or your lab paid for any analysis or figure software in the last year? What,
  how much, from which budget? Who signed off?
- What did you try instead of Prism? Why did you stop?

**22–27 Close the loop.**

- Of what we talked about, what costs you the most hours in a month?
- If that disappeared tomorrow, what would you do with the time?
- Is there anyone else I should talk to, a lab manager, a core facility, a PI?

**27–30 End.**
"Can I send you a build to try on that last data set? Can I follow up in two weeks?"

Avoid: "Would you use…?", "Would you pay…?", "Do you like…?", describing features before
they describe a problem. If they say "I'd pay for X", ask "What have you paid for X so
far?"

## Note template

Copy into `docs/launch/calls/YYYY-MM-DD-initials.md` (keep the folder out of git if it
holds names).

```text
Date:            Duration:
Who:             Role:          Field:         Institution type:
Source:          (where you found them)
OS:              Prism version / who pays:
Instrument(s):   Export format:

Last figure (what, when, steps, programs, time):

Last analysis (test, who chose, checks):

Batch work (how often, how many files, how done now):

Renewal / lost access story:

Money spent last 12 months (what, how much, whose budget, who approved):

Biggest time cost (their words):

Quotes (verbatim):
-
-

Referrals:
Follow-up (date, what to send):
Signal: [ ] pays now  [ ] has budget  [ ] batch work  [ ] regulated  [ ] Linux
```

After 10 calls, count the signal boxes and list the three problems named most often. That
list sets weeks 7–8.

## Where to find 10 users

Aim for 30 asks to get 10 calls. Mix PhD students, postdocs, lab managers and one core
facility.

- **r/labrats threads on lost Prism licences.** The threads cited in
  [LAB-TIER.md](../LAB-TIER.md) (sources 6, 7, 8, 13, 14): DM people who wrote about losing a
  licence, fitting many files, plate data or figure sizes. Only those who posted on the
  topic; one message each.
- **Your own launch threads.** Everyone who comments with a use case or a bug.
- **GitHub.** People who star the repo or open issues: reply on the issue, or email if
  their profile lists one.
- **University core facilities.** Flow cytometry, imaging and screening cores: the
  manager sees every lab's workflow. See [core-facilities.md](core-facilities.md).
- **Postdoc associations.** University postdoc associations and the National Postdoctoral
  Association; ask the organiser to forward one paragraph.
- **Departmental grad mailing lists.** Ask a student rep or administrator in your own or a
  friend's department to forward one paragraph.
- **Lab Slack and Discord communities.** Lab-wide or institute Slacks you or friends are
  in; science Discords with a #tools or #software channel. Read the rules; post once.
- **Bluesky and X.** Reply to #AcademicChatter and #labrats posts complaining about Prism,
  figure formatting or licences.
- **Friends of friends.** Each call ends with "who else should I talk to?"

## DM template

> Hi [name], I saw your post about [losing your Prism licence / fitting many CSV files /
> figure sizes]. I'm building a free open-source tool for lab stats and figures and I'm
> trying to understand how people actually do this today. Would you have 30 minutes for a
> call in the next two weeks? No pitch, just questions about your last figure. Happy to
> work around your schedule. [your name]

Forwardable paragraph (mailing lists, organisers):

> [Name], a [role] at [place], is looking for 10 researchers who make graphs or run stats
> in Prism or Excel for a 30-minute call about how they do it today. No sales, no
> preparation. Reply to [email] if you're willing.
