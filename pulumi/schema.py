"""
Generate architecture diagram.
"""

from diagrams import Cluster, Diagram, Edge
from diagrams.aws.compute import Lambda
from diagrams.aws.database import Dynamodb
from diagrams.aws.network import APIGateway
from diagrams.custom import Custom
from diagrams.gcp.devtools import Scheduler
from diagrams.generic.blank import Blank
from diagrams.generic.device import Mobile
from diagrams.saas.cdn import Cloudflare
from diagrams.saas.chat import Telegram

graph_attr = {
    "layout": "dot",
    "splines": "polyline",
    "compound": "true",
    "center": "true",
    "nodesep": "0.75",
    "ranksep": "1.0",
    "pad": "0.0",
    "margin": "0.0",
    "fontname": "Helvetica",
}

edge_attr = {
    "fontsize": "11",
    "fontname": "Helvetica",
    "labelfloat": "false",
}

node_attr = {
    "fontname": "Helvetica",
}

with Diagram(
    "",
    show=False,
    direction="LR",
    graph_attr=graph_attr,
    edge_attr=edge_attr,
    node_attr=node_attr,
    filename="erfiume",
):
    user = Mobile("user")
    bot = Telegram("erfiume_bot")
    cf = Cloudflare("erfiume.thedodo.xyz")
    api_er = Custom(
        "allertameteo.regione\n.emilia-romagna.it",
        icon_path="../assets/er-allertameteo.png",
    )
    api_marche = Custom(
        "app.protezionecivile\n.marche.it",
        icon_path="../assets/mc-protezionecivile.png",
    )
    right_spacer = Blank("", style="invis", width="0.000001", height="0.000001")
    with Cluster("AWS"):
        erfiume_bot = Lambda("erfiume_bot")
        erfiume_fetcher = Lambda("erfiume_fetcher")
        stations = Dynamodb("Regions-Stations")
        alerts = Dynamodb("Alerts")
        chats = Dynamodb("Chats")
        api_gw = APIGateway("API Gateway")
        scheduler = Scheduler("every 2h (20m emergency)")

    (
        user
        >> Edge(style="invis", weight="10")
        >> bot
        >> Edge(style="invis", weight="10")
        >> cf
    )
    api_er >> Edge(style="invis", weight="20") >> right_spacer
    api_marche >> Edge(style="invis", weight="20") >> right_spacer

    user >> Edge(label="send command") >> bot
    bot >> Edge(label="deliver messages") >> user
    bot >> Edge(label="trigger webhook", constraint="false") >> cf
    cf >> Edge(label="proxy request", weight="10", minlen="1") >> api_gw
    api_gw >> Edge(label="invoke lambda", minlen="2") >> erfiume_bot

    erfiume_bot >> Edge(label="read stations") >> stations
    erfiume_bot >> Edge(label="manage alerts") >> alerts
    erfiume_bot >> Edge(label="store chat region") >> chats
    erfiume_bot >> Edge(label="send replies") >> bot

    scheduler >> Edge(label="start") >> erfiume_fetcher
    erfiume_fetcher >> Edge(label="fetch data") >> api_er
    erfiume_fetcher >> Edge(label="fetch data") >> api_marche
    erfiume_fetcher >> Edge(label="upsert stations") >> stations
    erfiume_fetcher >> Edge(label="query / update alerts") >> alerts
    erfiume_fetcher >> Edge(label="send notifications") >> bot
