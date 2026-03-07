import os, re

for root, _, files in os.walk('src'):
    for file in files:
        if not file.endswith('.rs'): continue
        path = os.path.join(root, file)
        
        with open(path, 'r') as f:
            content = f.read()
            
        if 'models/message.rs' in path: continue
            
        lines = content.split('\n')
        out_lines = []
        for i, line in enumerate(lines):
            out_lines.append(line)
            if 'author_name:' in line and 'msg.author.name' not in line:
                # check if next line already has author_id
                if i+1 < len(lines) and 'author_id:' in lines[i+1]:
                    continue
                # insert author_id
                indent = line[:len(line) - len(line.lstrip())]
                out_lines.append(indent + 'author_id: "test".into(),')
                
        with open(path, 'w') as f:
            f.write('\n'.join(out_lines))
