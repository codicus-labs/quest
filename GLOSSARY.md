# Quest

Quest verbindet langfristige Ziele mit Aufgabenplanung und Zeiterfassung.

## Sprache

**Workspace**  
Ein Arbeitsbereich innerhalb eines Kontos oder lokalen Gastprofils mit eigenen Quests, Aufgaben, Zeiteinträgen und Wochenzielen.

**Aktiver Workspace**  
Der Arbeitsbereich, dessen Inhalte auf dem aktuellen Gerät angezeigt und bearbeitet werden.

**Inbox**  
Offene Aufgaben ohne Quest und ohne Plan-Datum. Eine Deadline allein hält eine Aufgabe in der Inbox.

**Today**  
Offene Aufgaben mit Plan-Datum oder Deadline heute oder früher, in manueller Tagesreihenfolge.

**Tagesreihenfolge**:
Die manuelle Reihenfolge der Aufgaben in Today und innerhalb eines Plan-Tags. Inbox und Quest-Listen haben eine eigene Reihenfolge.

**Plan-Datum** (`planned_on`)  
Der Tag, an dem man an einer Aufgabe arbeiten will. Liegt nie nach der Deadline. Eine Aufgabe mit vergangenem Plan-Datum ist *liegengeblieben*.

**Deadline** (`due_on`)  
Der Tag, bis zu dem eine Aufgabe erledigt sein muss. Eine Aufgabe mit vergangener Deadline ist *überfällig*. Nur der User verschiebt Daten, nie das System.

**Organizing**  
Erfasste Zeit für Organisatorisches zwischen Aufgaben, ohne Aufgabe und ohne Quest; merkt sich optional die Aufgabe, von der man kam.

**Postponement**  
Eine Verschiebung: Plan-Datum oder Deadline wurde auf einen späteren Tag gesetzt. Wird einmal geschrieben und nie geändert; Zähler werden daraus abgeleitet.

**Checklist**  
Abhakbare Punkte innerhalb einer Aufgabe. Wiederholte Aufgaben bekommen die Checkliste der Vorlage unabgehakt.

**Entfernte Begriffe**  
Mit dem Datenmodell 2 entfallen *Milestone* (Meilenstein), *Time block* (Zeitblock), der *Calendar* (Kalender mit Zeitraster) und die Quest-Beschreibung. Der frühere *Tag* einer Aufgabe heißt jetzt Plan-Datum.
